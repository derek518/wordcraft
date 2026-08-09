import { useState, useEffect, useCallback } from 'react'
import * as api from '../data/api'
import { playCorrect, playLevelUp } from '../core/sound'

interface SeasonTrackProps {
  onBack: () => void
}

const REDEEM_ITEMS = [
  { id: 'draw_ticket', label: '抽卡券', cost: 30, icon: '/assets/effects/star.png', hint: '开一张水晶卡牌' },
  { id: 'makeup_card', label: '补签卡', cost: 150, icon: '/assets/badges/badge_perfect.png', hint: '断签时自动消耗，保住连续天数' },
]

/** 里程碑配置 */
const MILESTONES = [
  { at: 3, label: '初出茅庐', icon: '/assets/ui/medal_bronze.png', color: '#cd7f32', desc: '完成3个时段' },
  { at: 7, label: '渐入佳境', icon: '/assets/ui/medal_silver.png', color: '#a0a0a0', desc: '完成7个时段' },
  { at: 14, label: '持之以恒', icon: '/assets/ui/medal_gold.png', color: '#ffd700', desc: '完成14个时段' },
  { at: 21, label: '完美一周', icon: '/assets/ui/crown.png', color: '#ff6b6b', desc: '完成全部时段' },
]

function formatDateRange(startStr: string): string {
  const start = new Date(startStr)
  const end = new Date(start)
  end.setDate(end.getDate() + 6)
  const fmt = (d: Date) => `${d.getMonth() + 1}/${d.getDate()}`
  return `${fmt(start)} - ${fmt(end)}`
}

export default function SeasonTrack({ onBack }: SeasonTrackProps) {
  const [season, setSeason] = useState<api.SeasonState | null>(null)
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const [flash, setFlash] = useState('')
  const [celebrateMilestone, setCelebrateMilestone] = useState<number | null>(null)

  const load = useCallback(async () => {
    setError('')
    try {
      const s = await api.getSeason()
      setSeason(s)
    } catch (e) {
      // 后端失败一律显示错误态。纯前端调试走 VITE_MOCK=1，见 src/data/devMock.ts
      setError(e instanceof Error ? e.message : String(e))
    }
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  const redeem = async (item: string, label: string) => {
    if (busy) return
    setBusy(true)
    setError('')
    try {
      // 这里曾有一条 DEV 分支直接本地假扣分、根本不调后端——
      // dev 下每次兑换都显示「已兑换」而实际什么都没发生
      await api.redeemPoints(item)
      playCorrect(0)
      setFlash(`已兑换 ${label}`)
      setTimeout(() => setFlash(''), 2000)
      await load()
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setBusy(false)
    }
  }

  const triggerMilestone = (at: number) => {
    playLevelUp()
    setCelebrateMilestone(at)
    setTimeout(() => setCelebrateMilestone(null), 2500)
  }

  if (!season) {
    return (
      <div className="flex items-center justify-center min-h-[400px]">
        {error ? (
          <div className="text-center max-w-md">
            <div className="text-4xl mb-4">🏁</div>
            <h2 className="text-xl font-bold mb-2">赛道无法载入</h2>
            <p className="text-wc-text-muted text-sm mb-6 break-words">{error}</p>
            <button onClick={onBack} className="px-6 py-2.5 btn-game bg-wc-surface-2 border border-wc-border rounded-xl font-bold hover:border-wc-primary transition">返回营地</button>
          </div>
        ) : (
          <div className="text-center">
            <div className="text-4xl mb-4 animate-pulse">🏁</div>
            <p className="text-wc-text-muted">正在铺设赛道…</p>
          </div>
        )}
      </div>
    )
  }

  const ahead = season.sessions_done - season.ghost_sessions
  const pct = Math.round(season.progress * 100)
  const ghostPct = Math.round(season.ghost_progress * 100)
  const dateRange = formatDateRange(season.week_start)

  // 已完成里程碑
  const nextMilestone = MILESTONES.find(m => season.sessions_done < m.at)

  /** 到某个时段数为止能拿的赛道积分。参数来自后端，前端不写死 */
  const pointsAt = (sessions: number) =>
    sessions * season.points_per_session +
    (sessions >= season.sessions_total ? season.perfect_bonus : 0)

  return (
    <div className="max-w-3xl mx-auto">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <button onClick={onBack} className="flex items-center gap-1 text-sm text-wc-text-dim hover:text-wc-text transition">
          <span className="text-lg">←</span> 返回
        </button>
        <div className="flex items-center gap-2">
          <span className="text-xl">🏁</span>
          <h2 className="text-xl font-bold font-game">本周赛道</h2>
        </div>
        <div className="text-xs font-game-mono text-wc-text-dim">{dateRange}</div>
      </div>

      {error && (
        <div className="p-3 rounded-xl bg-wc-danger/10 border border-wc-danger/30 text-sm mb-3">
          <span className="font-bold text-wc-danger">操作失败：</span>
          <span className="text-wc-text-dim ml-1 break-words">{error}</span>
        </div>
      )}

      {flash && (
        <div className="p-2 rounded-xl bg-wc-success/10 border border-wc-success/30 text-sm mb-3 text-wc-success text-center">
          {flash}
        </div>
      )}

      {/* 主赛道可视化 */}
      <div className="hud-panel rounded-2xl p-4 mb-3">
        {/* 顶部数据 */}
        <div className="flex items-center justify-between mb-4">
          <div>
            <div className="text-3xl font-bold font-game-mono text-wc-accent">{season.sessions_done}</div>
            <div className="text-xs text-wc-text-dim">/ {season.sessions_total} 时段完成</div>
          </div>
          <div className="text-center">
            <div className="text-xs text-wc-text-dim">预计积分</div>
            <div className="text-xl font-bold text-wc-gold font-game-mono">+{season.projected_points}</div>
          </div>
          <div className="text-right">
            <div className="text-xs text-wc-text-dim">当前积分</div>
            <div className="text-2xl font-bold text-wc-gold font-game-mono">{season.track_points}</div>
          </div>
        </div>

        {/* 赛道。两条道而非一条：跟自己比，接近才是常态，
            两台车叠在同一条上会糊成一团。上道是本周，下道是上周同期 */}
        <div className="mb-2">
          {/* 本周 */}
          <div className="flex items-center gap-2 mb-1">
            <span className="text-[10px] text-wc-accent font-bold w-12 shrink-0">本周</span>
            <div className="flex-1 h-8 bg-wc-bg rounded-full relative overflow-hidden border border-wc-border/40">
              <div
                className="absolute inset-y-1 left-1 rounded-full overflow-hidden transition-all duration-700"
                style={{ width: `calc(${season.progress * 100}% - 8px)` }}
              >
                <div className="h-full w-full progress-shine" />
              </div>

              {/* 里程碑刻度 */}
              {MILESTONES.map(m => {
                const pos = (m.at / season.sessions_total) * 100
                const reached = season.sessions_done >= m.at
                return (
                  <button
                    key={m.at}
                    onClick={() => reached && triggerMilestone(m.at)}
                    // 100% 处的刻度会被圆角容器切掉一半，往回收 14px
                    className="absolute top-1/2 -translate-y-1/2 transition-all"
                    style={{ left: `min(calc(${pos}% - 8px), calc(100% - 22px))` }}
                    title={m.desc}
                  >
                    <img
                      src={m.icon}
                      alt={m.label}
                      className={`w-5 h-5 object-contain ${reached ? 'grayscale-0' : 'grayscale opacity-30'}`}
                      style={{ imageRendering: 'pixelated' }}
                    />
                  </button>
                )
              })}

              <div
                className="absolute top-1/2 -translate-y-1/2 text-lg transition-all duration-700 z-10"
                style={{ left: `min(calc(${pct}% - 10px), calc(100% - 26px))` }}
              >
                <img
                  src="/assets/ui/racer.png"
                  alt=""
                  className="w-6 h-6 object-contain drop-shadow-[0_0_8px_rgba(6,182,212,0.6)]"
                  style={{ imageRendering: 'pixelated' }}
                />
              </div>
            </div>
          </div>

          {/* 上周同期。spec 明确排除社交对比，对手只能是上周的自己 */}
          <div className="flex items-center gap-2">
            <span className="text-[10px] text-wc-text-dim w-12 shrink-0">上周</span>
            <div className="flex-1 h-6 bg-wc-bg rounded-full relative overflow-hidden border border-wc-border/30 opacity-60">
              <div
                className="absolute inset-y-1 left-1 rounded-full bg-wc-surface-3 transition-all duration-700"
                style={{ width: `calc(${season.ghost_progress * 100}% - 8px)` }}
              />
              <div
                className="absolute top-1/2 -translate-y-1/2 transition-all duration-700 grayscale"
                style={{ left: `min(calc(${ghostPct}% - 8px), calc(100% - 22px))` }}
              >
                <img
                  src="/assets/ui/racer.png"
                  alt=""
                  className="w-5 h-5 object-contain"
                  style={{ imageRendering: 'pixelated' }}
                />
              </div>
            </div>
          </div>

          <div className="flex justify-between text-[10px] text-wc-text-dim mt-1 pl-14">
            <span>0%</span>
            <span className="text-wc-accent font-bold">{pct}%</span>
            <span>100%</span>
          </div>
        </div>

        {/* 对比文案 */}
        <div className="text-center text-xs mt-2">
          {ahead > 0 ? (
            <span className="text-wc-success">🚀 领先上周的自己 {ahead} 个时段</span>
          ) : ahead < 0 ? (
            <span className="text-wc-text-dim">💨 落后 {-ahead} 个时段，还有时间追上</span>
          ) : (
            <span className="text-wc-text-dim">➡️ 与上周持平</span>
          )}
        </div>
      </div>

      {/* 里程碑成就区 */}
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-2 mb-3">
        {MILESTONES.map(m => {
          const reached = season.sessions_done >= m.at
          const isNext = nextMilestone?.at === m.at
          return (
            <div
              key={m.at}
              className={`rounded-xl p-2 text-center border transition-all ${
                reached
                  ? 'border-wc-border/60 bg-wc-surface-2/60'
                  : isNext
                    ? 'border-wc-primary/40 bg-wc-primary/5 animate-pulse'
                    : 'border-wc-border/20 bg-wc-bg/40 opacity-50'
              }`}
            >
              <div className="mb-0.5">
                {/* 48px：32 格资产 1.5 倍下采样，绶带与星形还撑得住。
                    28px 时它们已经糊成一团 */}
                <img
                  src={m.icon}
                  alt={m.label}
                  className="w-12 h-12 mx-auto object-contain"
                  style={{ imageRendering: 'pixelated' }}
                />
              </div>
              <div className="text-[10px] font-bold truncate">{m.label}</div>
              {/* 标出到这里实际能拿多少分。只写「3时段」是个没有回报的刻度，
                  而积分本来就是按时段真金白银发的，说出来即可 */}
              <div className="text-[9px] text-wc-text-dim">
                {m.at}时段 · <span className="text-wc-gold font-game-mono">{pointsAt(m.at)}</span>分
              </div>
              {reached && <div className="text-[8px] text-wc-success mt-0.5">✓ 达成</div>}
              {isNext && !reached && (
                <div className="text-[8px] text-wc-primary mt-0.5">还差 {m.at - season.sessions_done}</div>
              )}
            </div>
          )
        })}
      </div>

      {/* 统计摘要卡 */}
      <div className="grid grid-cols-3 gap-2 mb-3">
        <div className="hud-panel rounded-xl p-3 text-center">
          <div className="text-lg font-bold font-game-mono text-wc-accent">{season.sessions_done}</div>
          <div className="text-[10px] text-wc-text-dim">已完成</div>
        </div>
        <div className="hud-panel rounded-xl p-3 text-center">
          <div className="text-lg font-bold font-game-mono text-wc-gold">{season.projected_points}</div>
          <div className="text-[10px] text-wc-text-dim">预计积分</div>
        </div>
        <div className="hud-panel rounded-xl p-3 text-center">
          <div className="text-lg font-bold font-game-mono text-wc-success">{season.sessions_total - season.sessions_done}</div>
          <div className="text-[10px] text-wc-text-dim">剩余时段</div>
        </div>
      </div>

      {/* 积分兑换 */}
      <div className="hud-panel rounded-2xl p-4">
        <div className="flex items-center justify-between mb-3">
          <span className="text-sm font-bold">🏪 积分兑换</span>
          <span className="text-xs text-wc-text-dim">每周日结算 · 断签不清空</span>
        </div>

        <div className="grid grid-cols-2 gap-3">
          {REDEEM_ITEMS.map(item => {
            const affordable = season.track_points >= item.cost
            return (
              <button
                key={item.id}
                onClick={() => affordable && redeem(item.id, item.label)}
                disabled={!affordable || busy}
                className={`rounded-xl p-3 text-center border transition ${
                  affordable
                    ? 'border-wc-border bg-wc-surface-2 hover:border-wc-primary cursor-pointer'
                    : 'border-wc-border/50 bg-wc-surface/50 opacity-50 cursor-default'
                }`}
              >
                <img src={item.icon} alt="" className="w-8 h-8 mx-auto object-contain mb-1" />
                <div className="text-sm font-bold">{item.label}</div>
                <div className="text-xs font-game-mono text-wc-gold">{item.cost} 分</div>
                <div className="text-[10px] text-wc-text-dim leading-tight mt-1">{item.hint}</div>
              </button>
            )
          })}
        </div>
      </div>

      {/* 里程碑庆祝弹窗 */}
      {celebrateMilestone !== null && (
        <div className="fixed inset-0 bg-black/80 flex items-center justify-center z-50 p-4" onClick={() => setCelebrateMilestone(null)}>
          <div className="text-center pop-in-bounce max-w-sm">
            {(() => {
              const m = MILESTONES.find(x => x.at === celebrateMilestone)
              if (!m) return null
              return (
                <>
                  <div className="mb-3" style={{ filter: `drop-shadow(0 0 20px ${m.color})` }}>
                    <img
                      src={m.icon}
                      alt={m.label}
                      className="w-20 h-20 mx-auto object-contain"
                      style={{ imageRendering: 'pixelated' }}
                    />
                  </div>
                  <h2 className="text-2xl font-bold mb-1" style={{ color: m.color }}>{m.label}</h2>
                  <p className="text-sm text-wc-text-dim mb-4">{m.desc}</p>
                  <div className="hud-panel rounded-xl p-3 mb-4 text-sm">
                    <div className="text-wc-gold font-bold">里程碑达成</div>
                    <div className="text-xs text-wc-text-dim mt-1">继续加油，向着下一个目标前进！</div>
                  </div>
                  <button onClick={() => setCelebrateMilestone(null)} className="px-8 py-3 btn-game bg-gradient-to-r from-wc-primary to-wc-primary-bright rounded-xl font-bold">太棒了</button>
                </>
              )
            })()}
          </div>
        </div>
      )}
    </div>
  )
}
