import { useState, useEffect, useCallback } from 'react'
import * as api from '../data/api'
import { playCorrect } from '../core/sound'

interface SeasonTrackProps {
  onBack: () => void
}

const REDEEM_ITEMS = [
  { id: 'draw_ticket', label: '抽卡券', cost: 30, icon: '/assets/effects/star.png',
    hint: '开一张水晶卡牌' },
  { id: 'makeup_card', label: '补签卡', cost: 150, icon: '/assets/badges/badge_perfect.png',
    hint: '断签时自动消耗，保住连续天数' },
]

/**
 * 赛季赛道。spec §4.2 F11。
 *
 * 幽灵车是上周的自己——spec 明确「无社交对比」。
 */
export default function SeasonTrack({ onBack }: SeasonTrackProps) {
  const [season, setSeason] = useState<api.SeasonState | null>(null)
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const [flash, setFlash] = useState('')

  const load = useCallback(async () => {
    setError('')
    try {
      setSeason(await api.getSeason())
    } catch (e) {
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

  if (!season) {
    return (
      <div className="flex items-center justify-center min-h-[400px]">
        {error ? (
          <div className="text-center max-w-md">
            <div className="text-4xl mb-4">🏁</div>
            <h2 className="text-xl font-bold mb-2">赛道无法载入</h2>
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
            <div className="text-4xl mb-4 animate-pulse">🏁</div>
            <p className="text-wc-text-muted">正在铺设赛道…</p>
          </div>
        )}
      </div>
    )
  }

  const ahead = season.sessions_done - season.ghost_sessions

  return (
    <div className="max-w-2xl mx-auto">
      <div className="flex items-center justify-between mb-5">
        <button
          onClick={onBack}
          className="flex items-center gap-1 text-sm text-wc-text-muted hover:text-wc-text transition"
        >
          <span className="text-lg">←</span> 返回
        </button>
        <h2 className="text-xl font-bold font-game">🏁 本周赛道</h2>
        <div className="text-sm font-game-mono text-wc-text-muted">
          {season.week_start.slice(5)} 起
        </div>
      </div>

      {error && (
        <div className="p-3 rounded-xl bg-wc-danger/10 border border-wc-danger/30 text-sm mb-4">
          <span className="font-bold text-wc-danger">操作失败：</span>
          <span className="text-wc-text-muted ml-1 break-words">{error}</span>
        </div>
      )}

      {flash && (
        <div className="p-3 rounded-xl bg-wc-success/10 border border-wc-success/30 text-sm mb-4 text-wc-success">
          {flash}
        </div>
      )}

      {/* 赛道 */}
      <div className="hud-panel rounded-2xl p-5 mb-4">
        <div className="flex items-baseline justify-between mb-4">
          <div>
            <span className="text-3xl font-bold font-game-mono text-wc-accent">
              {season.sessions_done}
            </span>
            <span className="text-wc-text-muted"> / {season.sessions_total} 时段</span>
          </div>
          <div className="text-right">
            <div className="text-xs text-wc-text-muted">本周可得</div>
            <div className="text-xl font-bold text-wc-gold font-game-mono">
              +{season.projected_points}
            </div>
          </div>
        </div>

        {/* 两条赛道：本周在上，幽灵车在下 */}
        <div className="space-y-3">
          <div>
            <div className="flex items-center justify-between text-xs mb-1">
              <span className="text-wc-accent font-bold">本周的你</span>
              <span className="font-game-mono text-wc-text-muted">
                {(season.progress * 100).toFixed(0)}%
              </span>
            </div>
            <div className="relative h-6 bg-wc-bg rounded-full overflow-hidden">
              <div
                className="absolute inset-y-0 left-0 progress-shine rounded-full transition-all duration-700"
                style={{ width: `${season.progress * 100}%` }}
              />
              <div
                className="absolute top-1/2 -translate-y-1/2 text-sm transition-all duration-700"
                style={{ left: `calc(${season.progress * 100}% - 10px)` }}
              >
                🏎️
              </div>
            </div>
          </div>

          <div>
            <div className="flex items-center justify-between text-xs mb-1">
              {/* 对手是上周的自己，spec 明确排除社交对比 */}
              <span className="text-wc-text-muted">上周同期</span>
              <span className="font-game-mono text-wc-text-muted">
                {(season.ghost_progress * 100).toFixed(0)}%
              </span>
            </div>
            <div className="relative h-6 bg-wc-bg rounded-full overflow-hidden opacity-60">
              <div
                className="absolute inset-y-0 left-0 bg-wc-surface-3 rounded-full"
                style={{ width: `${season.ghost_progress * 100}%` }}
              />
              <div
                className="absolute top-1/2 -translate-y-1/2 text-sm grayscale"
                style={{ left: `calc(${season.ghost_progress * 100}% - 10px)` }}
              >
                🏎️
              </div>
            </div>
          </div>
        </div>

        <div className="text-center text-sm mt-4">
          {ahead > 0 ? (
            <span className="text-wc-success">领先上周的自己 {ahead} 个时段</span>
          ) : ahead < 0 ? (
            <span className="text-wc-text-muted">落后 {-ahead} 个时段，还有时间追上</span>
          ) : (
            <span className="text-wc-text-muted">与上周持平</span>
          )}
        </div>
      </div>

      {/* 积分与兑换 */}
      <div className="hud-panel rounded-2xl p-5">
        <div className="flex items-center justify-between mb-4">
          <span className="text-sm font-bold">赛道积分</span>
          <span className="text-2xl font-bold text-wc-gold font-game-mono">
            {season.track_points}
          </span>
        </div>
        <p className="text-xs text-wc-text-muted mb-4">
          每周日结算发放 · 断签不清空积分
        </p>

        <div className="grid grid-cols-2 gap-3">
          {REDEEM_ITEMS.map((item) => {
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
                <div className="text-[10px] text-wc-text-muted leading-tight mt-1">
                  {item.hint}
                </div>
              </button>
            )
          })}
        </div>
      </div>
    </div>
  )
}
