import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'

interface AdventureMapProps {
  onStartTraining: (type: string) => void
  onOpenStats: () => void
  overallStats: any
}

const PORTALS = [
  { key: 'morning', name: '晨曦之门', icon: '🌅', time: '09:00-11:00', color: 'from-orange-400 to-yellow-300', element: '☀️' },
  { key: 'noon', name: '烈日之门', icon: '☀️', time: '13:00-15:00', color: 'from-yellow-400 to-amber-300', element: '🔥' },
  { key: 'evening', name: '星夜之门', icon: '🌙', time: '19:00-21:00', color: 'from-indigo-400 to-purple-300', element: '⭐' },
]

const ZONES = [
  { key: 'newbie', name: '新手村', words: 50, unlocked: true, element: 'none', color: '#e2e8f0' },
  { key: 'grass', name: '清风平原', words: 200, unlocked: false, element: 'grass', color: '#4ade80' },
  { key: 'water', name: '蓝水湖泊', words: 300, unlocked: false, element: 'water', color: '#3b82f6' },
  { key: 'fire', name: '赤焰山脉', words: 500, unlocked: false, element: 'fire', color: '#ef4444' },
  { key: 'thunder', name: '雷霆峡谷', words: 500, unlocked: false, element: 'thunder', color: '#a855f7' },
  { key: 'ice', name: '永冬之巅', words: 500, unlocked: false, element: 'ice', color: '#67e8f9' },
]

export default function AdventureMap({ onStartTraining, onOpenStats, overallStats }: AdventureMapProps) {
  const [nextSession, setNextSession] = useState<any>(null)
  const [completedToday] = useState<string[]>([])

  useEffect(() => {
    loadSessionInfo()
  }, [])

  const loadSessionInfo = async () => {
    try {
      const info = await invoke('get_next_session_time')
      setNextSession(info)
    } catch (e) {
      console.error(e)
    }
  }

  const getPortalStatus = (key: string) => {
    // In a real app, this would check the database for completed sessions today
    return completedToday.includes(key) ? 'completed' : 'available'
  }

  return (
    <div className="space-y-6">
      {/* Progress Overview */}
      <div className="grid grid-cols-2 gap-4">
        <div 
          onClick={onOpenStats}
          className="bg-wc-surface border border-wc-border rounded-xl p-4 cursor-pointer hover:border-wc-primary transition crystal-card"
        >
          <div className="text-wc-text-muted text-xs mb-1">词汇掌握</div>
          <div className="text-2xl font-bold">
            {overallStats ? overallStats.mastered : 0}
            <span className="text-sm text-wc-text-muted font-normal"> / {overallStats ? overallStats.total_words : 50}</span>
          </div>
          <div className="mt-2 h-2 bg-wc-surface-2 rounded-full overflow-hidden">
            <div 
              className="h-full bg-gradient-to-r from-wc-primary to-wc-accent rounded-full transition-all"
              style={{ width: `${overallStats ? (overallStats.mastered / Math.max(overallStats.total_words, 1)) * 100 : 0}%` }}
            />
          </div>
        </div>
        
        <div className="bg-wc-surface border border-wc-border rounded-xl p-4">
          <div className="text-wc-text-muted text-xs mb-1">下次传送门</div>
          {nextSession ? (
            <>
              <div className="text-xl font-bold">{nextSession.next_session}</div>
              <div className="text-sm text-wc-text-muted">
                {nextSession.minutes_until < 60 
                  ? `${nextSession.minutes_until} 分钟后`
                  : `${Math.floor(nextSession.minutes_until / 60)} 小时后`
                }
              </div>
            </>
          ) : (
            <div className="text-wc-text-muted">加载中...</div>
          )}
        </div>
      </div>

      {/* Portals */}
      <div>
        <h2 className="text-sm font-bold text-wc-text-muted uppercase tracking-wider mb-3">今日传送门</h2>
        <div className="grid grid-cols-3 gap-3">
          {PORTALS.map((portal) => {
            const status = getPortalStatus(portal.key)
            return (
              <button
                key={portal.key}
                onClick={() => status !== 'completed' && onStartTraining(portal.key)}
                disabled={status === 'completed'}
                className={`portal-btn relative rounded-xl p-4 text-center transition-all ${
                  status === 'completed'
                    ? 'bg-wc-surface-2 opacity-50 cursor-not-allowed'
                    : 'bg-wc-surface border border-wc-border hover:border-wc-primary hover:shadow-lg hover:shadow-wc-primary/20'
                }`}
              >
                <div className={`text-3xl mb-2 ${status === 'completed' ? 'grayscale' : ''}`}>
                  {status === 'completed' ? '✅' : portal.icon}
                </div>
                <div className="text-sm font-bold">{portal.name}</div>
                <div className="text-xs text-wc-text-muted mt-1">{portal.time}</div>
                
                {status !== 'completed' && (
                  <div className={`absolute inset-0 rounded-xl bg-gradient-to-br ${portal.color} opacity-0 hover:opacity-10 transition`} />
                )}
              </button>
            )
          })}
        </div>
      </div>

      {/* World Map - Zones */}
      <div>
        <h2 className="text-sm font-bold text-wc-text-muted uppercase tracking-wider mb-3">冒险地图</h2>
        <div className="bg-wc-surface border border-wc-border rounded-xl p-4">
          <div className="grid grid-cols-2 gap-3">
            {ZONES.map((zone) => (
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
                    style={{ backgroundColor: zone.unlocked ? zone.color : '#475569' }}
                  />
                </div>
                <div className="text-xs text-wc-text-muted">
                  {zone.unlocked ? `${zone.words} 词` : '🔒 被迷雾笼罩'}
                </div>
                {!zone.unlocked && (
                  <div className="mt-2 h-1.5 bg-wc-bg rounded-full overflow-hidden">
                    <div className="h-full bg-wc-text-muted/30 rounded-full" style={{ width: '0%' }} />
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Quick Actions */}
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
      </div>
    </div>
  )
}
