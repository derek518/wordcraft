import { useState, useEffect, useCallback } from 'react'
import * as api from '../data/api'
import type { OverallStats, Session, SessionType } from '../core/types'

interface AdventureMapProps {
  onStartTraining: (type: SessionType) => void
  onOpenStats: () => void
  onOpenAlbum: () => void
  stats: OverallStats | null
}

const PORTALS: { key: SessionType; name: string; icon: string; time: string; color: string }[] = [
  { key: 'morning', name: '晨曦之门', icon: '🌅', time: '09:00-11:00', color: 'from-orange-400 to-yellow-300' },
  { key: 'noon', name: '烈日之门', icon: '☀️', time: '13:00-15:00', color: 'from-yellow-400 to-amber-300' },
  { key: 'evening', name: '星夜之门', icon: '🌙', time: '19:00-21:00', color: 'from-indigo-400 to-purple-300' },
]

/** 区域配色。词数与解锁状态来自后端，这里只留视觉信息 */
const ZONE_COLORS: Record<string, string> = {
  newbie: '#e2e8f0',
  grass: '#4ade80',
  water: '#3b82f6',
  fire: '#ef4444',
  thunder: '#a855f7',
  ice: '#67e8f9',
}

export default function AdventureMap({ onStartTraining, onOpenStats, onOpenAlbum, stats }: AdventureMapProps) {
  const [sessions, setSessions] = useState<Session[]>([])
  const [zones, setZones] = useState<api.ZoneProgress[]>([])

  const loadSessions = useCallback(async () => {
    try {
      const [s, z] = await Promise.all([api.getTodaySessions(), api.getZoneProgress()])
      setSessions(s)
      setZones(z)
    } catch {
      // 拿不到时按空渲染，不阻断进入训练
      setSessions([])
      setZones([])
    }
  }, [])

  useEffect(() => {
    void loadSessions()
  }, [loadSessions])

  /** 传送门完成状态取自数据库，而非组件内永远为空的本地状态（审计 M7） */
  const isCompleted = (key: SessionType) =>
    sessions.some((s) => s.session_type === key && s.is_completed)

  const masteredRatio =
    stats && stats.total_words > 0
      ? ((stats.total_words - stats.untouched) / stats.total_words) * 100
      : 0

  return (
    <div className="space-y-6">
      <div className="grid grid-cols-2 gap-4">
        <button
          onClick={onOpenStats}
          className="bg-wc-surface border border-wc-border rounded-xl p-4 cursor-pointer hover:border-wc-primary transition crystal-card text-left"
        >
          <div className="text-wc-text-muted text-xs mb-1">已点亮水晶</div>
          <div className="text-2xl font-bold">
            {stats ? stats.total_words - stats.untouched : 0}
            <span className="text-sm text-wc-text-muted font-normal"> / {stats?.total_words ?? 0}</span>
          </div>
          <div className="mt-2 h-2 bg-wc-surface-2 rounded-full overflow-hidden">
            <div
              className="h-full bg-gradient-to-r from-wc-primary to-wc-accent rounded-full transition-all"
              style={{ width: `${masteredRatio}%` }}
            />
          </div>
        </button>

        <div className="bg-wc-surface border border-wc-border rounded-xl p-4">
          <div className="text-wc-text-muted text-xs mb-1">今日进度</div>
          <div className="text-2xl font-bold">
            {sessions.filter((s) => s.is_completed).length}
            <span className="text-sm text-wc-text-muted font-normal"> / 3 传送门</span>
          </div>
          <div className="text-sm text-wc-text-muted mt-1">
            {stats ? `累计 ${stats.total_reviews} 次冒险` : ''}
          </div>
        </div>
      </div>

      <div>
        <h2 className="text-sm font-bold text-wc-text-muted uppercase tracking-wider mb-3">今日传送门</h2>
        <div className="grid grid-cols-3 gap-3">
          {PORTALS.map((portal) => {
            const done = isCompleted(portal.key)
            return (
              <button
                key={portal.key}
                onClick={() => !done && onStartTraining(portal.key)}
                disabled={done}
                className={`portal-btn relative rounded-xl p-4 text-center transition-all ${
                  done
                    ? 'bg-wc-surface-2 opacity-50 cursor-not-allowed'
                    : 'bg-wc-surface border border-wc-border hover:border-wc-primary hover:shadow-lg hover:shadow-wc-primary/20'
                }`}
              >
                <div className={`text-3xl mb-2 ${done ? 'grayscale' : ''}`}>
                  {done ? '✅' : portal.icon}
                </div>
                <div className="text-sm font-bold">{portal.name}</div>
                <div className="text-xs text-wc-text-muted mt-1">{portal.time}</div>
              </button>
            )
          })}
        </div>
      </div>

      <div>
        <h2 className="text-sm font-bold text-wc-text-muted uppercase tracking-wider mb-3">冒险地图</h2>
        <div className="bg-wc-surface border border-wc-border rounded-xl p-4">
          <div className="grid grid-cols-2 gap-3">
            {zones.map((zone) => {
              const pct = zone.total > 0 ? (zone.learned / zone.total) * 100 : 0
              return (
                <div
                  key={zone.key}
                  className={`rounded-lg p-3 border transition-all ${
                    zone.unlocked
                      ? 'border-wc-border bg-wc-surface-2'
                      : 'border-wc-border/50 bg-wc-surface/50 opacity-60'
                  }`}
                >
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-sm font-bold">{zone.name}</span>
                    <span
                      className="w-3 h-3 rounded-full"
                      style={{
                        backgroundColor: zone.unlocked ? ZONE_COLORS[zone.key] : '#475569',
                      }}
                    />
                  </div>
                  {zone.unlocked ? (
                    <>
                      <div className="text-xs text-wc-text-muted mb-1.5">
                        {zone.learned} / {zone.total} 词
                      </div>
                      <div className="h-1.5 bg-wc-bg rounded-full overflow-hidden">
                        <div
                          className="h-full rounded-full transition-all"
                          style={{
                            width: `${pct}%`,
                            backgroundColor: ZONE_COLORS[zone.key],
                          }}
                        />
                      </div>
                    </>
                  ) : (
                    // 明示解锁条件而非只显示一把锁——阿斯伯格用户需要「可预测的全貌」
                    <div className="text-xs text-wc-text-muted">
                      🔒 Lv.{zone.required_level} 解锁
                    </div>
                  )}
                </div>
              )
            })}
          </div>
        </div>
      </div>

      <div className="flex gap-3">
        <button
          onClick={() => onStartTraining('free')}
          className="flex-1 py-3 bg-wc-surface-2 border border-wc-border rounded-lg text-sm font-bold hover:border-wc-primary transition"
        >
          ⚔️ 自由探险
        </button>
        <button
          onClick={onOpenStats}
          className="flex-1 py-3 bg-wc-surface-2 border border-wc-border rounded-lg text-sm font-bold hover:border-wc-primary transition"
        >
          📊 战绩面板
        </button>
        <button
          onClick={onOpenAlbum}
          className="flex-1 py-3 bg-wc-surface-2 border border-wc-border rounded-lg text-sm font-bold hover:border-wc-primary transition relative"
        >
          🎴 水晶图鉴
          {(stats?.draw_tickets ?? 0) > 0 && (
            <span className="absolute top-2 right-2 px-1.5 py-0.5 text-xs rounded-full bg-wc-gold text-wc-bg font-mono">
              {stats?.draw_tickets}
            </span>
          )}
        </button>
      </div>
    </div>
  )
}
