import { useState, useEffect, useCallback } from 'react'
import * as api from '../data/api'
import { playCorrect } from '../core/sound'

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
    hint: '词量里程碑奖励',
  },
  limited: {
    label: '限定方块',
    icon: '/assets/blocks/block_limited.png',
    hint: '连续打卡满 7 天 +1',
  },
}

/**
 * 家园建造。spec §4.2 F9。
 *
 * 欢迎页从第一天起就写着「收集的水晶可以用来建造家园」，此前却没有任何入口。
 */
export default function Homestead({ onBack }: HomesteadProps) {
  const [state, setState] = useState<api.HomesteadState | null>(null)
  const [selected, setSelected] = useState('normal')
  const [blueprints, setBlueprints] = useState<api.Blueprint[]>([])
  const [activeBp, setActiveBp] = useState<string | null>(null)
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)

  const load = useCallback(async () => {
    setError('')
    try {
      // 先补发再读取：用户可能在别处练了词，进家园时应该已经拿到方块，
      // 而不是要等下次启动
      await api.grantPendingBlocks()
      const [home, bps] = await Promise.all([api.getHomestead(), api.getBlueprints()])
      setState(home)
      setBlueprints(bps)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    }
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  /** 格子坐标 → 已放置的方块。网格只回传非空格，查表比遍历快 */
  const placedAt = new Map(state?.grid.map((b) => [`${b.x},${b.y}`, b.block_type]) ?? [])

  const blueprint = blueprints.find((b) => b.id === activeBp) ?? null
  /** 蓝图要求：坐标 → 应放的类型。轮廓叠加在空格上作为引导 */
  const planAt = new Map(blueprint?.cells.map((c) => [`${c.x},${c.y}`, c.block_type]) ?? [])

  /** 已按蓝图放对的格子数 */
  const matched = blueprint
    ? blueprint.cells.filter((c) => placedAt.get(`${c.x},${c.y}`) === c.block_type).length
    : 0

  const handleCell = async (x: number, y: number) => {
    if (busy || !state) return
    const existing = placedAt.get(`${x},${y}`)

    setBusy(true)
    setError('')
    try {
      // 后端返回完整快照，前端不自行推算库存增减——两处各算一遍必然会错开
      // 蓝图激活时按图纸要求的类型放置，省去用户在背包间来回切换
      const wanted = planAt.get(`${x},${y}`) ?? selected
      const next = existing
        ? await api.removeBlock(x, y)
        : await api.placeBlock(x, y, wanted)
      setState(next)
      if (!existing) playCorrect(0)
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
          const done = bp.cells.filter(
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
                  : 'border-wc-border bg-wc-surface-2 hover:border-wc-primary/50'
              }`}
            >
              {bp.name}
              <span className="ml-1.5 font-game-mono text-wc-text-muted">
                {done}/{bp.cells.length}
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
          <div className="flex gap-3 text-wc-text-muted">
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
        </div>
      )}

      {/* 网格。20×20 用 CSS grid 平铺，格子尺寸随容器自适应 */}
      <div className="hud-panel rounded-2xl p-4 mb-4 overflow-x-auto">
        <div
          className="grid gap-[2px] mx-auto"
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
            // 放对了：蓝图要求的类型与实际一致
            const correct = planned !== undefined && planned === block

            return (
              <button
                key={i}
                onClick={() => handleCell(x, y)}
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
                    ? 'hover:brightness-125 hover:scale-110'
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
                    className="w-full h-full object-contain"
                  />
                ) : (
                  planned && (
                    // 轮廓：淡显目标方块，引导而不代劳
                    <img
                      src={BLOCK_META[planned]?.icon}
                      alt=""
                      className="w-full h-full object-contain opacity-20"
                    />
                  )
                )}
              </button>
            )
          })}
        </div>
      </div>

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
        点空格放置 · 点方块移除后退回背包
      </p>
    </div>
  )
}
