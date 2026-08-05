import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'

interface StatsPanelProps {
  onBack: () => void
}

export default function StatsPanel({ onBack }: StatsPanelProps) {
  const [todayStats, setTodayStats] = useState<any>(null)
  const [overallStats, setOverallStats] = useState<any>(null)

  useEffect(() => {
    loadStats()
  }, [])

  const loadStats = async () => {
    try {
      const today = await invoke('get_today_stats')
      setTodayStats(today)
      
      const overall = await invoke('get_overall_stats')
      setOverallStats(overall)
    } catch (e) {
      console.error('Load stats error:', e)
    }
  }

  const getLevelTitle = (level: number) => {
    if (level < 5) return '迷路新手'
    if (level < 10) return '草原行者'
    if (level < 20) return '水晶猎人'
    if (level < 35) return '元素使徒'
    if (level < 50) return '遗忘克星'
    if (level < 100) return '词汇大师'
    return '传说冒险者'
  }

  return (
    <div className="max-w-lg mx-auto">
      {/* Header */}
      <div className="flex items-center gap-4 mb-6">
        <button 
          onClick={onBack}
          className="text-sm text-wc-text-muted hover:text-wc-text transition"
        >
          ← 返回
        </button>
        <h2 className="text-xl font-bold">📊 战绩面板</h2>
      </div>

      {/* Player Card */}
      {overallStats && (
        <div className="bg-gradient-to-br from-wc-primary/20 to-wc-accent/20 border border-wc-primary/30 rounded-xl p-6 mb-6">
          <div className="flex items-center justify-between mb-4">
            <div>
              <div className="text-sm text-wc-text-muted">冒险者等级</div>
              <div className="text-3xl font-bold">Lv.{overallStats.level}</div>
              <div className="text-sm text-wc-primary-bright">{getLevelTitle(overallStats.level)}</div>
            </div>
            <div className="text-right">
              <div className="text-sm text-wc-text-muted">总 XP</div>
              <div className="text-2xl font-bold text-wc-gold">{overallStats.total_xp}</div>
            </div>
          </div>
          
          <div className="h-2 bg-wc-surface rounded-full overflow-hidden mb-2">
            <div 
              className="h-full bg-gradient-to-r from-wc-primary to-wc-accent rounded-full"
              style={{ width: `${(overallStats.total_xp % 100)}%` }}
            />
          </div>
          <div className="text-xs text-wc-text-muted text-right">
            距离下一级还需 {100 - (overallStats.total_xp % 100)} XP
          </div>
        </div>
      )}

      {/* Streak */}
      {overallStats && (
        <div className="grid grid-cols-2 gap-4 mb-6">
          <div className="bg-wc-surface border border-wc-border rounded-xl p-4 text-center">
            <div className="text-3xl mb-1">🔥</div>
            <div className="text-2xl font-bold text-wc-fire">{overallStats.current_streak}</div>
            <div className="text-xs text-wc-text-muted">当前连续</div>
          </div>
          <div className="bg-wc-surface border border-wc-border rounded-xl p-4 text-center">
            <div className="text-3xl mb-1">🏆</div>
            <div className="text-2xl font-bold text-wc-gold">{overallStats.best_streak}</div>
            <div className="text-xs text-wc-text-muted">最佳记录</div>
          </div>
        </div>
      )}

      {/* Today's Stats */}
      {todayStats && (
        <div className="bg-wc-surface border border-wc-border rounded-xl p-4 mb-6">
          <h3 className="text-sm font-bold text-wc-text-muted uppercase tracking-wider mb-3">今日战绩</h3>
          <div className="grid grid-cols-4 gap-3 text-center">
            <div>
              <div className="text-xl font-bold">{todayStats.total}</div>
              <div className="text-xs text-wc-text-muted">总答题</div>
            </div>
            <div>
              <div className="text-xl font-bold text-wc-success">{todayStats.easy || 0}</div>
              <div className="text-xs text-wc-text-muted">轻松</div>
            </div>
            <div>
              <div className="text-xl font-bold text-wc-accent">{todayStats.good || 0}</div>
              <div className="text-xs text-wc-text-muted">掌握</div>
            </div>
            <div>
              <div className="text-xl font-bold text-wc-danger">{todayStats.again || 0}</div>
              <div className="text-xs text-wc-text-muted">需复习</div>
            </div>
          </div>
        </div>
      )}

      {/* Mastery Distribution */}
      {overallStats && (
        <div className="bg-wc-surface border border-wc-border rounded-xl p-4 mb-6">
          <h3 className="text-sm font-bold text-wc-text-muted uppercase tracking-wider mb-3">水晶分布</h3>
          <div className="space-y-3">
            {[
              { label: '灰暗水晶（未学）', count: overallStats.new_words, color: 'bg-gray-500' },
              { label: '微光水晶（学习中）', count: overallStats.learning, color: 'bg-wc-primary' },
              { label: '明亮水晶（复习中）', count: overallStats.review, color: 'bg-wc-accent' },
              { label: '闪耀水晶（已掌握）', count: overallStats.mastered, color: 'bg-wc-gold' },
            ].map((item) => {
              const total = Math.max(overallStats.total_words, 1)
              const pct = (item.count / total) * 100
              return (
                <div key={item.label}>
                  <div className="flex justify-between text-sm mb-1">
                    <span>{item.label}</span>
                    <span className="text-wc-text-muted">{item.count}</span>
                  </div>
                  <div className="h-2 bg-wc-bg rounded-full overflow-hidden">
                    <div className={`h-full ${item.color} rounded-full`} style={{ width: `${pct}%` }} />
                  </div>
                </div>
              )
            })}
          </div>
        </div>
      )}

      {/* Total Reviews */}
      {overallStats && (
        <div className="bg-wc-surface border border-wc-border rounded-xl p-4 text-center">
          <div className="text-sm text-wc-text-muted mb-1">累计复习次数</div>
          <div className="text-3xl font-bold">{overallStats.total_reviews}</div>
        </div>
      )}
    </div>
  )
}
