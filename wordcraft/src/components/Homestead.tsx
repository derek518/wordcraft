import { useState, useEffect, useCallback, useRef } from 'react'
import * as api from '../data/api'
import { playCorrect, playLevelUp } from '../core/sound'

interface HomesteadProps {
  onBack: () => void
}

const BLOCK_META: Record<string, { label: string; icon: string; hint: string }> = {
  normal: {
    label: '普通方块',
    icon: '/assets/blocks/block_normal.png',
    hint: '每答对一个新词 +1',
  },
  rare: {
    label: '稀有方块',
    icon: '/assets/blocks/block_rare.png',
    hint: '词量里程碑 · 击败魔王',
  },
  limited: {
    label: '限定方块',
    icon: '/assets/blocks/block_limited.png',
    hint: '连续打卡满 7 天 +1',
  },
}

/**
 * 居民会说的话。数字全部来自后端的真实状态，这里只负责措辞。
 *
 * 让住户报几个当下的数字，家园就同时是个软性的信息面板——
 * 建完之后仍有理由回来看一眼，而不用另做一套通知。
 */
function residentLines(d: api.HomesteadDigest): string[] {
  const lines: string[] = []
  if (d.due_count > 0) lines.push(`有 ${d.due_count} 个词该复习了。`)
  if (d.available_blocks > 0) lines.push(`仓库里还剩 ${d.available_blocks} 块没用。`)
  if (d.streak > 0) lines.push(`已经连着来了 ${d.streak} 天。`)
  if (d.words_to_milestone > 0) lines.push(`再学 ${d.words_to_milestone} 个词就能拿到稀有方块。`)
  if (lines.length === 0) lines.push('今天没什么要紧事，随便逛逛。')
  return lines
}

/**
 * 家园建造。spec §4.2 F9。
 *
 * 四张蓝图是一条扩建链而非四个选项：后一张严格包含前一张，
 * 建小屋花掉的方块在城堡里原样留着。
 */
export default function Homestead({ onBack }: HomesteadProps) {
  const [state, setState] = useState<api.HomesteadState | null>(null)
  const [selected, setSelected] = useState('normal')
  const [blueprints, setBlueprints] = useState<api.Blueprint[]>([])
  const [activeBp, setActiveBp] = useState<string | null>(null)
  const [res, setRes] = useState<api.ResidentsState | null>(null)
  const [picking, setPicking] = useState<number | null>(null)
  const [celebrating, setCelebrating] = useState<api.Blueprint | null>(null)
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)

  /** 上一次已知的完成集合。用来识别「刚刚建成」而非「本来就建好了」 */
  const knownDone = useRef<Set<string> | null>(null)
  /** 拖动中：按下不放划过多个格子 */
  const painting = useRef(false)

  const load = useCallback(async () => {
    setError('')
    try {
      // 先补发再读取：用户可能在别处练了词，进家园时应该已经拿到方块
      await api.grantPendingBlocks()
      const [home, bps, r] = await Promise.all([
        api.getHomestead(),
        api.getBlueprints(),
        api.getResidents(),
      ])
      setState(home)
      setBlueprints(bps)
      setRes(r)
      knownDone.current = new Set(r.completed)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    }
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  // 松开鼠标就结束涂抹，即便指针已经离开网格
  useEffect(() => {
    const stop = () => {
      painting.current = false
    }
    window.addEventListener('pointerup', stop)
    return () => window.removeEventListener('pointerup', stop)
  }, [])

  /** 格子坐标 → 已放置的方块。网格只回传非空格，查表比遍历快 */
  const placedAt = new Map(state?.grid.map((b) => [`${b.x},${b.y}`, b.block_type]) ?? [])

  const blueprint = blueprints.find((b) => b.id === activeBp) ?? null
  const planAt = new Map(blueprint?.cells.map((c) => [`${c.x},${c.y}`, c.block_type]) ?? [])
  const matched = blueprint
    ? blueprint.cells.filter((c) => placedAt.get(`${c.x},${c.y}`) === c.block_type).length
    : 0

  /** 刷新居民，并在有新蓝图建成时庆祝一次 */
  const refreshResidents = useCallback(async () => {
    try {
      const r = await api.getResidents()
      const before = knownDone.current
      setRes(r)
      knownDone.current = new Set(r.completed)

      // 首次加载时 before 为 null——那时的「已完成」是历史成果，不该弹庆祝
      if (before) {
        const fresh = r.completed.find((id) => !before.has(id))
        if (fresh) {
          const bp = blueprints.find((b) => b.id === fresh)
          if (bp) {
            playLevelUp()
            setCelebrating(bp)
          }
        }
      }
    } catch {
      // 居民刷新失败不该影响建造本身
    }
  }, [blueprints])

  const touchCell = async (x: number, y: number, remove: boolean) => {
    if (!state) return
    const existing = placedAt.get(`${x},${y}`)
    // 涂抹时只做一种操作：来回划过同一格反复放了又拆，是最容易误操作的地方
    if (remove ? !existing : !!existing) return

    setBusy(true)
    setError('')
    try {
      // 后端返回完整快照，前端不自行推算库存增减
      // 蓝图激活时按图纸要求的类型放置，省去在背包间来回切换
      const wanted = planAt.get(`${x},${y}`) ?? selected
      const next = remove ? await api.removeBlock(x, y) : await api.placeBlock(x, y, wanted)
      setState(next)
      if (!remove) playCorrect(0)
      await refreshResidents()
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
      painting.current = false
    } finally {
      setBusy(false)
    }
  }

  const assign = async (slot: number, cardId: number) => {
    setBusy(true)
    setError('')
    try {
      setRes(await api.moveInResident(slot, cardId))
      playCorrect(0)
      setPicking(null)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setBusy(false)
    }
  }

  const evict = async (slot: number) => {
    setBusy(true)
    try {
      setRes(await api.moveOutResident(slot))
      setPicking(null)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setBusy(false)
    }
  }

  if (!state) {
    return (
      <div className="flex items-center justify-center min-h-[400px]">
        {error ? (
          <div className="text-center max-w-md">
            <div className="text-4xl mb-4">🏚️</div>
            <h2 className="text-xl font-bold mb-2">家园无法载入</h2>
            <p className="text-wc-text-muted text-sm mb-6 break-words">{error}</p>
            <button
              onClick={onBack}
              className="px-6 py-2.5 btn-game bg-wc-surface-2 border border-wc-border rounded-xl font-bold hover:border-wc-primary transition"
            >
              返回营地
            </button>
          </div>
        ) : (
          <div className="text-center">
            <div className="text-4xl mb-4 animate-pulse">🏠</div>
            <p className="text-wc-text-muted">正在搭建地基…</p>
          </div>
        )}
      </div>
    )
  }

  const totalPlaced = state.grid.length
  const totalOwned = state.inventory.reduce((sum, s) => sum + s.owned, 0)
  const lines = res ? residentLines(res.digest) : []

  return (
    <div className="max-w-3xl mx-auto">
      <div className="flex items-center justify-between mb-5">
        <button
          onClick={onBack}
          className="flex items-center gap-1 text-sm text-wc-text-muted hover:text-wc-text transition"
        >
          <span className="text-lg">←</span> 返回
        </button>
        <div className="flex items-center gap-2">
          <img src="/assets/blocks/block_normal.png" alt="" className="w-6 h-6 object-contain" />
          <h2 className="text-xl font-bold font-game">我的家园</h2>
        </div>
        <div className="text-sm font-game-mono text-wc-text-muted">
          {totalPlaced}/{totalOwned}
        </div>
      </div>

      {error && (
        <div className="p-3 rounded-xl bg-wc-danger/10 border border-wc-danger/30 text-sm mb-4">
          <span className="font-bold text-wc-danger">操作失败：</span>
          <span className="text-wc-text-muted ml-1 break-words">{error}</span>
        </div>
      )}

      {totalOwned === 0 && (
        <div className="hud-panel rounded-2xl p-5 mb-4 text-center">
          <p className="text-sm text-wc-text-muted">
            还没有方块。<span className="text-wc-text">每答对一个新词就能获得一块</span>，
            去传送门练几轮再回来。
          </p>
        </div>
      )}

      {/* 蓝图。轮廓引导而非自动摆放——点一下就铺好会把建造变成领奖 */}
      <div className="flex gap-2 mb-3 overflow-x-auto pb-1">
        <button
          onClick={() => setActiveBp(null)}
          className={`px-3 py-2 rounded-lg text-xs whitespace-nowrap border transition ${
            activeBp === null
              ? 'border-wc-primary bg-wc-primary/15'
              : 'border-wc-border bg-wc-surface-2 hover:border-wc-primary/50'
          }`}
        >
          自由建造
        </button>
        {blueprints.map((bp) => {
          const done = res?.completed.includes(bp.id) ?? false
          const built = bp.cells.filter(
            (c) => placedAt.get(`${c.x},${c.y}`) === c.block_type,
          ).length
          return (
            <button
              key={bp.id}
              onClick={() => setActiveBp(activeBp === bp.id ? null : bp.id)}
              title={bp.description}
              className={`px-3 py-2 rounded-lg text-xs whitespace-nowrap border transition ${
                activeBp === bp.id
                  ? 'border-wc-primary bg-wc-primary/15'
                  : done
                    ? 'border-wc-success/50 bg-wc-success/10 hover:border-wc-success'
                    : 'border-wc-border bg-wc-surface-2 hover:border-wc-primary/50'
              }`}
            >
              {done && <span className="mr-1">✓</span>}
              {bp.name}
              <span className="ml-1.5 font-game-mono text-wc-text-muted">
                {built}/{bp.cells.length}
              </span>
            </button>
          )
        })}
      </div>

      {blueprint && (
        <div className="hud-panel rounded-xl p-3 mb-3 text-xs">
          <div className="flex items-center justify-between mb-2">
            <span className="text-wc-text-muted">{blueprint.description}</span>
            <span className="font-game-mono text-wc-accent">
              {matched}/{blueprint.cells.length}
            </span>
          </div>
          <div className="h-1.5 bg-wc-bg rounded-full overflow-hidden mb-2">
            <div
              className="h-full progress-shine rounded-full transition-all duration-500"
              style={{ width: `${(matched / blueprint.cells.length) * 100}%` }}
            />
          </div>
          <div className="flex gap-3 flex-wrap text-wc-text-muted">
            {blueprint.required.map(([type, need]) => {
              const have = state.inventory.find((s) => s.block_type === type)?.owned ?? 0
              return (
                <span key={type} className={have < need ? 'text-wc-warning' : ''}>
                  {BLOCK_META[type]?.label} {need} 块
                  {have < need && `（还差 ${need - have}）`}
                </span>
              )
            })}
          </div>
          {blueprint.stage > 1 && (
            // 扩建链最容易被误解的一点：以为要拆了重来
            <p className="mt-2 text-wc-text-muted/80">
              在前一阶段的基础上扩建，已放好的方块不用动。
            </p>
          )}
        </div>
      )}

      {/* 网格。按住可连续涂抹——逐格点 24 次建小屋是纯摩擦 */}
      <div className="hud-panel rounded-2xl p-4 mb-4 overflow-x-auto">
        <div
          className="grid gap-[2px] mx-auto select-none"
          style={{
            gridTemplateColumns: `repeat(${state.grid_size}, minmax(0, 1fr))`,
            maxWidth: 'min(100%, 640px)',
          }}
        >
          {Array.from({ length: state.grid_size * state.grid_size }, (_, i) => {
            const x = i % state.grid_size
            const y = Math.floor(i / state.grid_size)
            const block = placedAt.get(`${x},${y}`)
            const planned = planAt.get(`${x},${y}`)
            const correct = planned !== undefined && planned === block

            return (
              <button
                key={i}
                onPointerDown={(e) => {
                  e.preventDefault()
                  painting.current = true
                  void touchCell(x, y, !!block)
                }}
                onPointerEnter={() => {
                  // 涂抹方向由起点决定：起点是空格就一路放，是方块就一路拆
                  if (painting.current && !busy) void touchCell(x, y, !!block)
                }}
                disabled={busy}
                title={
                  block
                    ? `点击移除（${BLOCK_META[block]?.label}）`
                    : planned
                      ? `蓝图：${BLOCK_META[planned]?.label}`
                      : `放置到 (${x}, ${y})`
                }
                className={`relative aspect-square rounded-[3px] transition-all ${
                  block
                    ? 'hover:brightness-125'
                    : planned
                      ? 'bg-wc-primary/15 border border-wc-primary/50 hover:bg-wc-primary/30'
                      : 'bg-wc-surface-2/40 hover:bg-wc-primary/25 border border-wc-border/30'
                } ${correct ? 'ring-1 ring-wc-success/60' : ''} ${
                  busy ? 'cursor-wait' : 'cursor-pointer'
                }`}
              >
                {block ? (
                  <img
                    src={BLOCK_META[block]?.icon}
                    alt={BLOCK_META[block]?.label}
                    className="w-full h-full object-contain pointer-events-none"
                  />
                ) : (
                  planned && (
                    // 轮廓：淡显目标方块，引导而不代劳
                    <img
                      src={BLOCK_META[planned]?.icon}
                      alt=""
                      className="w-full h-full object-contain opacity-20 pointer-events-none"
                    />
                  )
                )}
              </button>
            )
          })}
        </div>
      </div>

      {/* 居民。建成蓝图才解锁位置——这是建造第一次真正产生结果 */}
      {res && (
        <div className="hud-panel rounded-2xl p-4 mb-4">
          <div className="flex items-center justify-between mb-3">
            <span className="text-sm font-bold">🏘️ 居民</span>
            <span className="text-xs font-game-mono text-wc-text-muted">
              {res.residents.length}/{res.slots} 位 · 全部建成可住 {res.max_slots} 位
            </span>
          </div>

          {res.slots === 0 ? (
            <p className="text-xs text-wc-text-muted text-center py-3">
              建成一张蓝图就能请一只生物住进来。
              <br />
              小屋只需 {blueprints[0]?.cells.length ?? 0} 块普通方块。
            </p>
          ) : (
            <>
              <div className="flex gap-3 flex-wrap">
                {Array.from({ length: res.slots }, (_, slot) => {
                  const who = res.residents.find((r) => r.slot === slot)
                  return (
                    <button
                      key={slot}
                      onClick={() => setPicking(slot)}
                      disabled={busy}
                      className={`w-20 rounded-xl p-2 text-center border transition ${
                        who
                          ? 'border-wc-border bg-wc-surface-2 hover:border-wc-primary'
                          : 'border-dashed border-wc-border bg-wc-bg/40 hover:border-wc-primary/60'
                      }`}
                    >
                      {who ? (
                        <>
                          <img
                            src={who.image_path}
                            alt={who.name}
                            className="w-12 h-12 mx-auto object-contain"
                          />
                          <div className="text-[11px] font-bold truncate">{who.name}</div>
                        </>
                      ) : (
                        <>
                          <div className="w-12 h-12 mx-auto flex items-center justify-center text-2xl opacity-40">
                            ＋
                          </div>
                          <div className="text-[11px] text-wc-text-muted">空位</div>
                        </>
                      )}
                    </button>
                  )
                })}
              </div>

              {/* 住户转述的真实数字，让家园顺带成为一块信息面板 */}
              {res.residents.length > 0 && (
                <div className="mt-3 pt-3 border-t border-wc-border/50 space-y-1">
                  {res.residents.slice(0, lines.length).map((r, i) => (
                    <div key={r.slot} className="text-xs text-wc-text-muted">
                      <span className="text-wc-accent">{r.name}</span>：{lines[i]}
                    </div>
                  ))}
                </div>
              )}
            </>
          )}
        </div>
      )}

      {/* 方块背包 */}
      <div className="grid grid-cols-3 gap-3">
        {state.inventory.map((stock) => {
          const meta = BLOCK_META[stock.block_type]
          const isSelected = selected === stock.block_type
          const usable = stock.available > 0
          return (
            <button
              key={stock.block_type}
              onClick={() => usable && setSelected(stock.block_type)}
              disabled={!usable}
              className={`hud-panel rounded-xl p-3 text-center transition-all border-2 ${
                isSelected && usable
                  ? 'border-wc-primary'
                  : 'border-transparent hover:border-wc-border-bright'
              } ${usable ? 'cursor-pointer' : 'opacity-40 cursor-default'}`}
            >
              <img
                src={meta?.icon}
                alt={meta?.label}
                className="w-12 h-12 mx-auto object-contain mb-1"
              />
              <div className="text-xs font-bold">{meta?.label}</div>
              <div className="text-lg font-game-mono text-wc-gold">{stock.available}</div>
              <div className="text-[10px] text-wc-text-muted leading-tight mt-0.5">
                {meta?.hint}
              </div>
            </button>
          )
        })}
      </div>

      <p className="text-xs text-wc-text-muted text-center mt-4">
        点空格放置 · 按住可连续涂抹 · 点方块移除后退回背包
      </p>

      {/* 选择住户 */}
      {picking !== null && res && (
        <div
          className="fixed inset-0 bg-black/80 flex items-center justify-center z-50 p-4"
          onClick={() => setPicking(null)}
        >
          <div
            className="bg-wc-surface border border-wc-border rounded-2xl p-5 max-w-md w-full pop-in"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className="text-lg font-bold font-game text-center mb-1">谁来住这里</h3>
            <p className="text-xs text-wc-text-muted text-center mb-4">
              只有收集到的生物能入住 · 画作请去图鉴欣赏
            </p>

            {res.candidates.length === 0 ? (
              <p className="text-sm text-wc-text-muted text-center py-6">
                还没有可入住的生物。
                <br />
                去水晶图鉴抽一张吧。
              </p>
            ) : (
              <div className="grid grid-cols-4 gap-2 max-h-64 overflow-y-auto">
                {res.candidates.map((c) => (
                  <button
                    key={c.card_id}
                    onClick={() => void assign(picking, c.card_id)}
                    disabled={busy}
                    className="rounded-xl p-2 text-center border border-wc-border bg-wc-surface-2 hover:border-wc-primary transition"
                  >
                    <img
                      src={c.image_path}
                      alt={c.name}
                      className="w-12 h-12 mx-auto object-contain"
                    />
                    <div className="text-[11px] truncate">{c.name}</div>
                  </button>
                ))}
              </div>
            )}

            <div className="flex gap-2 mt-4">
              {res.residents.some((r) => r.slot === picking) && (
                <button
                  onClick={() => void evict(picking)}
                  className="flex-1 py-2 text-sm rounded-lg border border-wc-border bg-wc-surface-2 hover:border-wc-danger transition"
                >
                  请它搬走
                </button>
              )}
              <button
                onClick={() => setPicking(null)}
                className="flex-1 py-2 text-sm text-wc-text-muted hover:text-wc-text transition"
              >
                取消
              </button>
            </div>
          </div>
        </div>
      )}

      {/* 建成庆祝。改版前放下最后一块什么都不会发生 */}
      {celebrating && (
        <div
          className="fixed inset-0 bg-black/85 flex items-center justify-center z-50 p-4"
          onClick={() => setCelebrating(null)}
        >
          <div className="text-center pop-in max-w-sm">
            <div className="text-6xl mb-4">🏆</div>
            <h2 className="text-2xl font-bold mb-1">{celebrating.name}建成了</h2>
            <p className="text-sm text-wc-text-muted mb-5">{celebrating.description}</p>
            <div className="hud-panel rounded-xl p-4 mb-6 text-sm">
              <div className="text-wc-gold font-bold">解锁一个入住位</div>
              <div className="text-xs text-wc-text-muted mt-1">
                请一只收集到的生物住进来
              </div>
            </div>
            <button
              onClick={() => setCelebrating(null)}
              className="px-8 py-3 btn-game bg-gradient-to-r from-wc-primary to-wc-primary-bright rounded-xl font-bold hover:opacity-90 transition"
            >
              去看看
            </button>
          </div>
        </div>
      )}
    </div>
  )
}
