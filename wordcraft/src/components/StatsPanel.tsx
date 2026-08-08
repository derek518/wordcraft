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

function levelBadge(level: number): string {
  if (level < 5) return '/assets/badges/badge_sprout.png'
  if (level < 10) return '/assets/badges/badge_fire.png'
  if (level < 20) return '/assets/badges/badge_sword.png'
  if (level < 35) return '/assets/badges/badge_builder.png'
  if (level < 50) return '/assets/badges/badge_collector.png'
  if (level < 100) return '/assets/badges/badge_perfect.png'
  return '/assets/badges/badge_night.png'
}

const MASTERY_META = [
  { key: 'untouched', label: '灰暗水晶', sub: '未学', color: '#475569', glow: 'rgba(71, 85, 105, 0.3)', icon: '/assets/crystals/crystal_rock_dim.png' },
  { key: 'learning', label: '微光水晶', sub: '学习中', color: '#7c3aed', glow: 'rgba(124, 58, 237, 0.3)', icon: '/assets/crystals/crystal_water_faint.png' },
  { key: 'reinforcing', label: '闪烁水晶', sub: '强化中', color: '#f97316', glow: 'rgba(249, 115, 22, 0.3)', icon: '/assets/crystals/crystal_fire_faint.png' },
  { key: 'review', label: '明亮水晶', sub: '复习中', color: '#06b6d4', glow: 'rgba(6, 182, 212, 0.3)', icon: '/assets/crystals/crystal_grass_bright.png' },
  { key: 'mastered', label: '传说水晶', sub: '已掌握', color: '#fbbf24', glow: 'rgba(251, 191, 36, 0.3)', icon: '/assets/crystals/crystal_ice_bright.png' },
]

export default function StatsPanel({ onBack }: StatsPanelProps) {
  const [today, setToday] = useState<DayStats | null>(null)
  const [overall, setOverall] = useState<OverallStats | null>(null)
  const [mastery, setMastery] = useState<MasteryDistribution | null>(null)
  const [heatmap, setHeatmap] = useState<api.HeatmapCell[]>([])
  const [error, setError] = useState('')

  const load = useCallback(async () => {
    setError('')
    try {
      const [t, o, m, h] = await Promise.all([
        api.getTodayStats(),
        api.getOverallStats(),
        api.getMasteryDistribution(),
        // 12 周：足以看出习惯，又不会把格子挤到看不清
        api.getHeatmap(84),
      ])
      setToday(t)
      setOverall(o)
      setMastery(m)
      setHeatmap(h)
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
      {/* 返回按钮 */}
      <div className="flex items-center gap-4 mb-6">
        <button
          onClick={onBack}
          className="text-sm text-wc-text-muted hover:text-wc-text transition flex items-center gap-1"
        >
          <span>←</span> 返回营地
        </button>
        <h2 className="text-xl font-bold tracking-wide">战绩面板</h2>
      </div>

      {error && (
        <div className="p-3 rounded-xl bg-wc-danger/10 border border-wc-danger/30 text-sm mb-6">
          <span className="font-bold text-wc-danger">读取失败：</span>
          <span className="text-wc-text-muted ml-1 break-words">{error}</span>
          <button onClick={load} className="ml-2 underline hover:text-wc-text">
            重试
          </button>
        </div>
      )}

      {/* ===== 等级卡片 ===== */}
      {overall && progress && (
        <div
          className="rounded-2xl p-6 mb-6 relative overflow-hidden"
          style={{
            background: 'linear-gradient(135deg, rgba(124, 58, 237, 0.15), rgba(6, 182, 212, 0.1))',
            border: '1px solid rgba(124, 58, 237, 0.3)',
            boxShadow: '0 0 30px rgba(124, 58, 237, 0.1), inset 0 1px 0 rgba(255,255,255,0.05)',
          }}
        >
          {/* 背景光晕 */}
          <div className="absolute -right-10 -top-10 w-40 h-40 rounded-full blur-3xl opacity-20 bg-wc-primary" />

          <div className="relative flex items-center gap-5">
            {/* 徽章图标 */}
            <div className="relative flex-shrink-0">
              <img
                src={levelBadge(progress.level)}
                alt={levelTitle(progress.level)}
                className="w-20 h-20 object-contain drop-shadow-[0_0_15px_rgba(168,85,247,0.4)]"
              />
              <div className="absolute -bottom-1 -right-1 px-2 py-0.5 rounded-full bg-wc-primary text-white text-xs font-bold font-game-mono">
                Lv.{progress.level}
              </div>
            </div>

            <div className="flex-1 min-w-0">
              <div className="text-sm text-wc-text-muted mb-1">冒险者称号</div>
              <div className="text-2xl font-bold mb-1 tracking-wide">{levelTitle(progress.level)}</div>
              <div className="flex items-center gap-3">
                <div className="flex-1 h-2.5 bg-wc-bg rounded-full overflow-hidden">
                  <div
                    className="h-full rounded-full transition-all duration-700"
                    style={{
                      width: `${progress.ratio * 100}%`,
                      background: 'linear-gradient(90deg, #7c3aed, #a855f7, #06b6d4)',
                      boxShadow: '0 0 10px rgba(124, 58, 237, 0.5)',
                    }}
                  />
                </div>
                <span className="text-xs text-wc-text-muted font-game-mono whitespace-nowrap">
                  {progress.current}/{progress.needed} XP
                </span>
              </div>
            </div>

            <div className="text-right flex-shrink-0">
              <div className="text-xs text-wc-text-muted mb-1">总 XP</div>
              <div className="text-2xl font-bold text-wc-gold font-game-mono">{overall.total_xp}</div>
            </div>
          </div>
        </div>
      )}

      {/* ===== 核心数据 ===== */}
      {overall && (
        <div className="grid grid-cols-2 gap-3 mb-6">
          <div
            className="rounded-xl p-4 text-center relative overflow-hidden"
            style={{
              background: 'linear-gradient(135deg, rgba(239, 68, 68, 0.1), rgba(251, 191, 36, 0.05))',
              border: '1px solid rgba(239, 68, 68, 0.2)',
            }}
          >
            <div className="relative">
              <div className="text-3xl mb-1">🔥</div>
              <div className="text-3xl font-bold text-wc-fire font-game-mono">{overall.current_streak}</div>
              <div className="text-xs text-wc-text-muted mt-1">当前连续</div>
            </div>
          </div>
          <div
            className="rounded-xl p-4 text-center relative overflow-hidden"
            style={{
              background: 'linear-gradient(135deg, rgba(251, 191, 36, 0.1), rgba(245, 158, 11, 0.05))',
              border: '1px solid rgba(251, 191, 36, 0.2)',
            }}
          >
            <div className="relative">
              <div className="text-3xl mb-1">🏆</div>
              <div className="text-3xl font-bold text-wc-gold font-game-mono">{overall.best_streak}</div>
              <div className="text-xs text-wc-text-muted mt-1">最佳记录</div>
            </div>
          </div>
        </div>
      )}

      {/* ===== 今日战绩 ===== */}
      {today && (
        <div className="hud-panel rounded-2xl p-5 mb-6">
          <h3 className="text-xs font-bold text-wc-text-muted uppercase tracking-[0.15em] mb-4 flex items-center gap-2">
            <img src="/assets/effects/star.png" alt="" className="w-4 h-4 object-contain" />
            今日战绩
          </h3>
          <div className="grid grid-cols-4 gap-3 text-center">
            {[
              { value: today.total, label: '总答题', color: 'text-wc-text' },
              { value: today.easy, label: '轻松', color: 'text-wc-success' },
              { value: today.good, label: '掌握', color: 'text-wc-accent' },
              { value: today.again, label: '需复习', color: 'text-wc-danger' },
            ].map((item) => (
              <div
                key={item.label}
                className="p-3 rounded-xl bg-wc-bg/50 border border-wc-border/30"
              >
                <div className={`text-2xl font-bold font-game-mono ${item.color}`}>{item.value}</div>
                <div className="text-xs text-wc-text-muted mt-1">{item.label}</div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* ===== 水晶分布 ===== */}
      {heatmap.length > 0 && (
        <div className="hud-panel rounded-2xl p-5 mb-6">
          <h3 className="text-xs font-bold text-wc-text-muted uppercase tracking-[0.15em] mb-4 flex items-center gap-2">
            <img src="/assets/effects/sparkle.png" alt="" className="w-4 h-4 object-contain" />
            近 12 周
          </h3>
          {/* 按周分列。grid-flow-col 让日期竖着排、周横着走，
              与常见的贡献热力图一致 */}
          <div className="grid grid-rows-7 grid-flow-col gap-[3px] justify-center">
            {heatmap.map((cell) => {
              // 分档而非线性映射：一天答 5 题和 50 题的差别，
              // 线性会让绝大多数格子挤在最暗的一档
              const level =
                cell.count === 0 ? 0 : cell.count < 5 ? 1 : cell.count < 15 ? 2 : cell.count < 30 ? 3 : 4
              const colors = [
                'rgba(60,60,100,0.25)',
                'rgba(124,58,237,0.35)',
                'rgba(124,58,237,0.6)',
                'rgba(168,85,247,0.85)',
                'rgba(6,182,212,1)',
              ]
              return (
                <div
                  key={cell.date}
                  title={`${cell.date}　${cell.count} 题`}
                  className="w-[9px] h-[9px] rounded-[2px] transition-transform hover:scale-150"
                  style={{ backgroundColor: colors[level] }}
                />
              )
            })}
          </div>
          <div className="flex items-center justify-end gap-1 mt-3 text-[10px] text-wc-text-muted">
            <span>少</span>
            {['rgba(60,60,100,0.25)','rgba(124,58,237,0.35)','rgba(124,58,237,0.6)','rgba(168,85,247,0.85)','rgba(6,182,212,1)'].map((c) => (
              <span key={c} className="w-[9px] h-[9px] rounded-[2px]" style={{ backgroundColor: c }} />
            ))}
            <span>多</span>
          </div>
        </div>
      )}

      {mastery && (
        <div className="hud-panel rounded-2xl p-5 mb-6">
          <h3 className="text-xs font-bold text-wc-text-muted uppercase tracking-[0.15em] mb-4 flex items-center gap-2">
            <img src="/assets/crystals/crystal_fire_bright.png" alt="" className="w-4 h-4 object-contain" />
            水晶分布
          </h3>
          <div className="space-y-4">
            {MASTERY_META.map((meta) => {
              const count = (mastery as any)[meta.key] as number
              const pct = mastery.total > 0 ? (count / mastery.total) * 100 : 0
              return (
                <div key={meta.key} className="group">
                  <div className="flex items-center justify-between text-sm mb-1.5">
                    <div className="flex items-center gap-2">
                      <img src={meta.icon} alt="" className="w-5 h-5 object-contain" />
                      <span>{meta.label}</span>
                      <span className="text-xs text-wc-text-muted">{meta.sub}</span>
                    </div>
                    <span className="font-game-mono text-wc-text-muted">{count}</span>
                  </div>
                  <div className="h-2.5 bg-wc-bg rounded-full overflow-hidden relative">
                    <div
                      className="h-full rounded-full transition-all duration-700"
                      style={{
                        width: `${pct}%`,
                        backgroundColor: meta.color,
                        boxShadow: pct > 0 ? `0 0 8px ${meta.glow}` : 'none',
                      }}
                    />
                  </div>
                </div>
              )
            })}
          </div>
          {/* 总数 */}
          <div className="mt-4 pt-4 border-t border-wc-border/30 text-center">
            <span className="text-xs text-wc-text-muted">水晶总数 </span>
            <span className="font-bold font-game-mono">{mastery.total}</span>
          </div>
        </div>
      )}

      {/* ===== 累计复习 ===== */}
      {overall && (
        <div
          className="rounded-2xl p-5 text-center relative overflow-hidden"
          style={{
            background: 'linear-gradient(135deg, rgba(6, 182, 212, 0.08), rgba(124, 58, 237, 0.08))',
            border: '1px solid rgba(6, 182, 212, 0.2)',
          }}
        >
          <div className="relative">
            <div className="text-xs text-wc-text-muted mb-2">累计复习次数</div>
            <div className="text-4xl font-bold font-game-mono tracking-wider">{overall.total_reviews}</div>
            <div className="text-xs text-wc-text-muted mt-2">
              词汇量估计：约 <span className="text-wc-accent font-bold">{overall.vocab_estimate}</span> 词
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
