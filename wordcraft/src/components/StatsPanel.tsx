import { useState, useEffect, useCallback } from 'react'
import * as api from '../data/api'
import { levelProgress } from '../core/progression'
import type { DayStats, MasteryDistribution, OverallStats } from '../core/types'

interface StatsPanelProps {
  onBack: () => void
}

function levelTitle(level: number): string {
  if (level < 5) return '迷路新手'
  if (level < 10) return '草原行者'
  if (level < 20) return '水晶猎人'
  if (level < 35) return '元素使徒'
  if (level < 50) return '遗忘克星'
  if (level < 100) return '词汇大师'
  return '传说冒险者'
}

export default function StatsPanel({ onBack }: StatsPanelProps) {
  const [today, setToday] = useState<DayStats | null>(null)
  const [overall, setOverall] = useState<OverallStats | null>(null)
  const [mastery, setMastery] = useState<MasteryDistribution | null>(null)
  const [error, setError] = useState('')

  const load = useCallback(async () => {
    setError('')
    try {
      const [t, o, m] = await Promise.all([
        api.getTodayStats(),
        api.getOverallStats(),
        api.getMasteryDistribution(),
      ])
      setToday(t)
      setOverall(o)
      setMastery(m)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    }
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  const progress = overall ? levelProgress(overall.total_xp) : null

  return (
    <div className="max-w-lg mx-auto">
      <div className="flex items-center gap-4 mb-6">
        <button onClick={onBack} className="text-sm text-wc-text-muted hover:text-wc-text transition">
          ← 返回
        </button>
        <h2 className="text-xl font-bold">📊 战绩面板</h2>
      </div>

      {error && (
        <div className="p-3 rounded-lg bg-wc-danger/10 border border-wc-danger/30 text-sm mb-6">
          <span className="font-bold text-wc-danger">读取失败：</span>
          <span className="text-wc-text-muted ml-1 break-words">{error}</span>
          <button onClick={load} className="ml-2 underline hover:text-wc-text">
            重试
          </button>
        </div>
      )}

      {overall && progress && (
        <div className="bg-gradient-to-br from-wc-primary/20 to-wc-accent/20 border border-wc-primary/30 rounded-xl p-6 mb-6">
          <div className="flex items-center justify-between mb-4">
            <div>
              <div className="text-sm text-wc-text-muted">冒险者等级</div>
              <div className="text-3xl font-bold">Lv.{progress.level}</div>
              <div className="text-sm text-wc-primary-bright">{levelTitle(progress.level)}</div>
            </div>
            <div className="text-right">
              <div className="text-sm text-wc-text-muted">总 XP</div>
              <div className="text-2xl font-bold text-wc-gold">{overall.total_xp}</div>
            </div>
          </div>

          <div className="h-2 bg-wc-surface rounded-full overflow-hidden mb-2">
            <div
              className="h-full bg-gradient-to-r from-wc-primary to-wc-accent rounded-full transition-all"
              style={{ width: `${progress.ratio * 100}%` }}
            />
          </div>
          <div className="text-xs text-wc-text-muted text-right">
            {progress.needed > 0
              ? `距离下一级还需 ${progress.needed - progress.current} XP`
              : '已达最高等级'}
          </div>
        </div>
      )}

      {overall && (
        <div className="grid grid-cols-2 gap-4 mb-6">
          <div className="bg-wc-surface border border-wc-border rounded-xl p-4 text-center">
            <div className="text-3xl mb-1">🔥</div>
            <div className="text-2xl font-bold text-wc-fire">{overall.current_streak}</div>
            <div className="text-xs text-wc-text-muted">当前连续</div>
          </div>
          <div className="bg-wc-surface border border-wc-border rounded-xl p-4 text-center">
            <div className="text-3xl mb-1">🏆</div>
            <div className="text-2xl font-bold text-wc-gold">{overall.best_streak}</div>
            <div className="text-xs text-wc-text-muted">最佳记录</div>
          </div>
        </div>
      )}

      {today && (
        <div className="bg-wc-surface border border-wc-border rounded-xl p-4 mb-6">
          <h3 className="text-sm font-bold text-wc-text-muted uppercase tracking-wider mb-3">今日战绩</h3>
          <div className="grid grid-cols-4 gap-3 text-center">
            <div>
              <div className="text-xl font-bold">{today.total}</div>
              <div className="text-xs text-wc-text-muted">总答题</div>
            </div>
            <div>
              <div className="text-xl font-bold text-wc-success">{today.easy}</div>
              <div className="text-xs text-wc-text-muted">轻松</div>
            </div>
            <div>
              <div className="text-xl font-bold text-wc-accent">{today.good}</div>
              <div className="text-xs text-wc-text-muted">掌握</div>
            </div>
            <div>
              <div className="text-xl font-bold text-wc-danger">{today.again}</div>
              <div className="text-xs text-wc-text-muted">需复习</div>
            </div>
          </div>
        </div>
      )}

      {mastery && (
        <div className="bg-wc-surface border border-wc-border rounded-xl p-4 mb-6">
          <h3 className="text-sm font-bold text-wc-text-muted uppercase tracking-wider mb-3">水晶分布</h3>
          <div className="space-y-3">
            {[
              { label: '灰暗水晶（未学）', count: mastery.untouched, color: 'bg-gray-500' },
              { label: '微光水晶（学习中）', count: mastery.learning, color: 'bg-wc-primary' },
              { label: '闪烁水晶（强化中）', count: mastery.reinforcing, color: 'bg-wc-warning' },
              { label: '明亮水晶（复习中）', count: mastery.review, color: 'bg-wc-accent' },
              { label: '传说水晶（已掌握）', count: mastery.mastered, color: 'bg-wc-gold' },
            ].map((item) => {
              const pct = mastery.total > 0 ? (item.count / mastery.total) * 100 : 0
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

      {overall && (
        <div className="bg-wc-surface border border-wc-border rounded-xl p-4 text-center">
          <div className="text-sm text-wc-text-muted mb-1">累计复习次数</div>
          <div className="text-3xl font-bold">{overall.total_reviews}</div>
        </div>
      )}
    </div>
  )
}
