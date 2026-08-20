import { useState, useEffect, useCallback } from 'react'
import * as api from '../data/api'
import type { DrillMode } from '../core/question'
import type { OverallStats, Session, SessionType } from '../core/types'

interface AdventureMapProps {
  onStartTraining: (type: SessionType, drill?: DrillMode) => void
  onOpenStats: () => void
  onOpenAlbum: () => void
  onOpenHomestead: () => void
  onOpenSeason: () => void
  onOpenBoss: () => void
  onOpenLibrary: () => void
  stats: OverallStats | null
}

/**
 * 自由练习的三种模式。spec §4.2 F8「不限量刷词、拼写专项、听写模式」。
 *
 * 拼写与听写只是**强制题型**，队列与积分与普通自由练习完全相同——
 * 它们不是独立玩法，是同一套练习的不同考查角度。
 */
const DRILL_OPTIONS: { mode: DrillMode; label: string; icon: string; hint: string }[] = [
  { mode: null, label: '不限量刷词', icon: '⚔️', hint: '按每个词当前的掌握程度出题' },
  { mode: 'spelling', label: '拼写专项', icon: '✍️', hint: '全部改为看释义拼单词' },
  { mode: 'dictation', label: '听写模式', icon: '🎧', hint: '全部改为听发音选词' },
]

const PORTALS: { key: SessionType; name: string; time: string; image: string; color: string; glowColor: string }[] = [
  {
    key: 'morning',
    name: '晨曦之门',
    time: '09:00–11:00',
    image: '/assets/portals/portal_morning.png',
    color: 'from-orange-500/20 to-yellow-500/20',
    glowColor: 'rgba(251, 146, 60, 0.5)',
  },
  {
    key: 'noon',
    name: '烈日之门',
    time: '13:00–15:00',
    image: '/assets/portals/portal_noon.png',
    color: 'from-yellow-500/20 to-amber-500/20',
    glowColor: 'rgba(245, 158, 11, 0.5)',
  },
  {
    key: 'evening',
    name: '星夜之门',
    time: '19:00–21:00',
    image: '/assets/portals/portal_evening.png',
    color: 'from-indigo-500/20 to-purple-500/20',
    glowColor: 'rgba(168, 85, 247, 0.5)',
  },
]

const ZONE_META: Record<string, { icon: string; element: string; desc: string }> = {
  newbie: { icon: '🏠', element: 'neutral', desc: '冒险的起点' },
  grass: { icon: '🌿', element: 'grass', desc: '初中基础词' },
  water: { icon: '💧', element: 'water', desc: '初中核心词' },
  fire: { icon: '🔥', element: 'fire', desc: '高中核心词' },
  thunder: { icon: '⚡', element: 'thunder', desc: '高中拓展词' },
  ice: { icon: '❄️', element: 'ice', desc: '高考高频词' },
  rock: { icon: '🪨', element: 'rock', desc: '高考难点词' },
}

const ELEMENT_COLORS: Record<string, { bg: string; glow: string; text: string }> = {
  neutral: { bg: '#64748b', glow: 'rgba(100, 116, 139, 0.4)', text: '#94a3b8' },
  grass: { bg: '#22c55e', glow: 'rgba(74, 222, 128, 0.4)', text: '#4ade80' },
  water: { bg: '#3b82f6', glow: 'rgba(59, 130, 246, 0.4)', text: '#60a5fa' },
  fire: { bg: '#ef4444', glow: 'rgba(239, 68, 68, 0.4)', text: '#f87171' },
  thunder: { bg: '#a855f7', glow: 'rgba(168, 85, 247, 0.4)', text: '#c084fc' },
  ice: { bg: '#22d3ee', glow: 'rgba(103, 232, 249, 0.4)', text: '#67e8f9' },
  rock: { bg: '#f59e0b', glow: 'rgba(245, 158, 11, 0.4)', text: '#fbbf24' },
}

export default function AdventureMap({ onStartTraining, onOpenStats, onOpenAlbum, onOpenHomestead, onOpenSeason, onOpenBoss, onOpenLibrary, stats }: AdventureMapProps) {
  const [sessions, setSessions] = useState<Session[]>([])
  const [zones, setZones] = useState<api.ZoneProgress[]>([])
  const [hoveredPortal, setHoveredPortal] = useState<string | null>(null)
  const [pickingDrill, setPickingDrill] = useState(false)
  // 听写模式需要发音。关掉 TTS 还让人选，进去只会得到一道无声的题
  const [audioOn, setAudioOn] = useState(true)
  const [loadError, setLoadError] = useState('')

  const loadSessions = useCallback(async () => {
    setLoadError('')
    try {
      const [s, z, tts] = await Promise.all([
        api.getTodaySessions(),
        api.getZoneProgress(),
        api.getSetting('tts_provider'),
      ])
      setSessions(s)
      setZones(z)
      setAudioOn(tts !== 'off')
    } catch (e) {
      setLoadError(e instanceof Error ? e.message : String(e))
    }
  }, [])

  useEffect(() => {
    void loadSessions()
  }, [loadSessions])

  const isCompleted = (key: SessionType) =>
    sessions.some((s) => s.session_type === key && s.is_completed)

  const completedCount = sessions.filter((s) => s.is_completed).length
  const masteredRatio = stats && stats.total_words > 0
    ? ((stats.total_words - stats.untouched) / stats.total_words) * 100
    : 0

  return (
    <div className="space-y-3 relative">
      {loadError && (
        <div className="p-3 rounded-xl bg-wc-danger/10 border border-wc-danger/30 text-sm hud-panel">
          <span className="font-bold text-wc-danger">营地无法载入：</span>
          <span className="text-wc-text-muted ml-1 break-words">{loadError}</span>
          <button
            onClick={() => void loadSessions()}
            className="ml-3 text-wc-accent hover:underline"
          >
            重试
          </button>
        </div>
      )}
      {/* ===== 顶部 HUD ===== */}
      <div className="grid grid-cols-2 gap-3">
        {/* 词汇掌握 */}
        <button
          onClick={onOpenStats}
          className="hud-panel rounded-xl p-3 cursor-pointer transition-all duration-300 hover:scale-[1.02] text-left group"
          style={{ borderColor: 'rgba(60, 60, 100, 0.8)' }}
        >
          <div className="flex items-center gap-2 mb-1">
            <img src="/assets/crystals/crystal_fire_bright.png" alt="" className="w-5 h-5 object-contain crystal-shimmer" />
            <span className="text-wc-text-muted text-[10px] font-bold uppercase tracking-wider">已点亮水晶</span>
          </div>
          <div className="text-2xl font-bold font-game-mono">
            {stats ? stats.total_words - stats.untouched : 0}
            <span className="text-sm text-wc-text-muted font-normal font-game"> / {stats?.total_words ?? 0}</span>
          </div>
          <div className="mt-2 h-2 bg-wc-bg rounded-full overflow-hidden relative">
            <div
              className="h-full progress-shine rounded-full transition-all duration-700"
              style={{ width: `${masteredRatio}%` }}
            />
          </div>
        </button>

        {/* 今日进度 */}
        <div className="hud-panel rounded-xl p-3" style={{ borderColor: 'rgba(60, 60, 100, 0.8)' }}>
          <div className="flex items-center gap-2 mb-1">
            <img src="/assets/ui/chest_small.png" alt="" className="w-5 h-5 object-contain" />
            <span className="text-wc-text-muted text-[10px] font-bold uppercase tracking-wider">今日进度</span>
          </div>
          <div className="flex items-end gap-2">
            <div className="text-2xl font-bold font-game-mono">{completedCount}</div>
            <div className="text-sm text-wc-text-muted mb-0.5">/ 3 传送门</div>
          </div>
          {/* 三颗传送门状态指示灯 */}
          <div className="flex gap-2 mt-2">
            {PORTALS.map((p) => {
              const done = isCompleted(p.key)
              return (
                <div
                  key={p.key}
                  className={`flex-1 h-2 rounded-full transition-all duration-500 ${
                    done
                      ? 'bg-gradient-to-r from-wc-success to-wc-accent'
                      : 'bg-wc-bg'
                  }`}
                  style={done ? { boxShadow: `0 0 6px ${p.glowColor}` } : {}}
                />
              )
            })}
          </div>
          {stats && stats.total_reviews > 0 && (
            <div className="text-[10px] text-wc-text-muted mt-1.5">
              累计 {stats.total_reviews} 次冒险
            </div>
          )}
        </div>
      </div>

      {/* ===== 传送门 ===== */}
      <div>
        <h2 className="text-[10px] font-bold text-wc-text-muted uppercase tracking-[0.15em] mb-2 flex items-center gap-2">
          <span className="w-4 h-[1.5px] bg-wc-primary/60 rounded-full" />
          今日传送门
          <span className="w-4 h-[1.5px] bg-wc-primary/60 rounded-full" />
        </h2>
        <div className="grid grid-cols-3 gap-3">
          {PORTALS.map((portal) => {
            const done = isCompleted(portal.key)
            const isHovered = hoveredPortal === portal.key
            return (
              <button
                key={portal.key}
                onClick={() => !done && onStartTraining(portal.key)}
                disabled={done}
                onMouseEnter={() => setHoveredPortal(portal.key)}
                onMouseLeave={() => setHoveredPortal(null)}
                className={`relative rounded-xl overflow-hidden transition-all duration-300 ${
                  done
                    ? 'opacity-50 cursor-not-allowed'
                    : 'cursor-pointer hover:scale-[1.04] hover:-translate-y-0.5'
                }`}
                style={{
                  boxShadow: isHovered && !done
                    ? `0 0 20px ${portal.glowColor}, 0 4px 16px rgba(0,0,0,0.4)`
                    : done
                      ? 'none'
                      : `0 2px 8px rgba(0,0,0,0.3)`,
                }}
              >
                {/* 背景渐变 */}
                <div className={`absolute inset-0 bg-gradient-to-b ${portal.color}`} />

                {/* 边框光 */}
                <div
                  className="absolute inset-0 rounded-xl"
                  style={{
                    border: isHovered && !done ? `2px solid ${portal.glowColor}` : '1px solid rgba(255,255,255,0.08)',
                    transition: 'border 0.3s',
                  }}
                />

                <div className="relative p-3 text-center">
                  {/* Portal 图片 */}
                  <div className="relative w-14 h-14 mx-auto mb-1.5">
                    <img
                      src={portal.image}
                      alt={portal.name}
                      className={`w-full h-full object-contain transition-all duration-300 ${
                        done ? 'grayscale opacity-50' : isHovered ? 'scale-110' : ''
                      }`}
                      style={!done ? { filter: isHovered ? `drop-shadow(0 0 8px ${portal.glowColor})` : 'none' } : {}}
                    />
                    {done && (
                      <div className="absolute inset-0 flex items-center justify-center">
                        <span className="text-2xl">✅</span>
                      </div>
                    )}
                  </div>

                  <div className="text-sm font-bold tracking-wide">{portal.name}</div>
                  <div className="text-[11px] text-wc-text-muted mt-0.5 font-game-mono">{portal.time}</div>

                  {/* 完成标记 */}
                  {done && (
                    <div className="mt-1 text-[10px] text-wc-success font-bold">已完成</div>
                  )}
                  {!done && isHovered && (
                    <div className="mt-1 text-[10px] text-wc-accent font-bold animate-pulse">点击进入 →</div>
                  )}
                </div>
              </button>
            )
          })}
        </div>
      </div>

      {/* ===== 冒险地图 ===== */}
      <div>
        <h2 className="text-[10px] font-bold text-wc-text-muted uppercase tracking-[0.15em] mb-2 flex items-center gap-2">
          <span className="w-4 h-[1.5px] bg-wc-primary/60 rounded-full" />
          冒险地图
          <span className="w-4 h-[1.5px] bg-wc-primary/60 rounded-full" />
        </h2>
        <div className="hud-panel rounded-xl p-3" style={{ borderColor: 'rgba(60, 60, 100, 0.8)' }}>
          <div className="grid grid-cols-2 gap-2">
            {zones.map((zone) => {
              const meta = ZONE_META[zone.key] || ZONE_META.newbie
              const colors = ELEMENT_COLORS[meta.element] || ELEMENT_COLORS.neutral
              const pct = zone.total > 0 ? (zone.learned / zone.total) * 100 : 0

              return (
                <div
                  key={zone.key}
                  className={`relative rounded-lg p-3 border transition-all duration-300 overflow-hidden ${
                    zone.unlocked
                      ? 'border-wc-border/80 hover:border-wc-border-bright hover:scale-[1.02]'
                      : 'border-wc-border/40 opacity-60'
                  }`}
                  style={zone.unlocked ? { '--zone-glow': colors.glow } as React.CSSProperties : {}}
                >
                  {/* 元素背景光 */}
                  {zone.unlocked && (
                    <div
                      className="absolute -right-6 -top-6 w-16 h-16 rounded-full blur-xl opacity-25"
                      style={{ background: colors.bg }}
                    />
                  )}

                  <div className="relative flex items-center justify-between mb-1.5">
                    <div className="flex items-center gap-1.5">
                      <span className="text-base">{meta.icon}</span>
                      <span className="text-sm font-bold">{zone.name}</span>
                    </div>
                    <div
                      className="w-2.5 h-2.5 rounded-full"
                      style={{
                        backgroundColor: colors.bg,
                        boxShadow: zone.unlocked ? `0 0 6px ${colors.glow}` : 'none',
                      }}
                    />
                  </div>

                  {zone.unlocked ? (
                    <>
                      <div className="flex items-center justify-between text-[11px] mb-1.5">
                        <span className="text-wc-text-dim">{meta.desc}</span>
                        <span className="font-game-mono text-wc-text">{zone.learned}/{zone.total}</span>
                      </div>
                      <div className="h-1.5 bg-wc-bg rounded-full overflow-hidden">
                        <div
                          className="h-full rounded-full transition-all duration-700"
                          style={{
                            width: `${pct}%`,
                            backgroundColor: colors.bg,
                            boxShadow: pct > 0 ? `0 0 6px ${colors.glow}` : 'none',
                          }}
                        />
                      </div>
                    </>
                  ) : (
                    <div className="flex items-center gap-1 text-[11px] text-wc-text-muted">
                      <span>🔒</span>
                      <span>Lv.{zone.required_level} 解锁</span>
                    </div>
                  )}
                </div>
              )
            })}
          </div>
        </div>
      </div>

      {/* ===== 快捷操作 ===== */}
      <div className="grid grid-cols-3 gap-2">
        <button
          onClick={() => setPickingDrill(true)}
          className="btn-game hud-panel rounded-lg py-2.5 text-sm font-bold transition-all hover:border-wc-primary/50 flex items-center justify-center gap-1.5"
          style={{ borderColor: 'rgba(60, 60, 100, 0.8)' }}
        >
          <img src="/assets/ui/boss.png" alt="" className="w-4 h-4 object-contain" />
          自由探险
        </button>
        <button
          onClick={onOpenStats}
          className="btn-game hud-panel rounded-lg py-2.5 text-sm font-bold transition-all hover:border-wc-primary/50 flex items-center justify-center gap-1.5"
          style={{ borderColor: 'rgba(60, 60, 100, 0.8)' }}
        >
          <img src="/assets/effects/star.png" alt="" className="w-4 h-4 object-contain" />
          战绩面板
        </button>
        <button
          onClick={onOpenHomestead}
          className="btn-game hud-panel rounded-lg py-2.5 text-sm font-bold transition-all hover:border-wc-primary/50 flex items-center justify-center gap-1.5"
          style={{ borderColor: 'rgba(60, 60, 100, 0.8)' }}
        >
          <img src="/assets/blocks/block_normal.png" alt="" className="w-4 h-4 object-contain" />
          我的家园
        </button>
        <button
          onClick={onOpenLibrary}
          className="btn-game hud-panel rounded-lg py-2.5 text-sm font-bold transition-all hover:border-wc-primary/50 flex items-center justify-center gap-1.5"
          style={{ borderColor: 'rgba(60, 60, 100, 0.8)' }}
        >
          <span className="text-base leading-none">🔍</span>
          水晶图谱
        </button>
        <button
          onClick={onOpenBoss}
          className="btn-game hud-panel rounded-lg py-2.5 text-sm font-bold transition-all hover:border-wc-primary/50 flex items-center justify-center gap-1.5"
          style={{ borderColor: 'rgba(60, 60, 100, 0.8)' }}
        >
          <span className="text-base leading-none">⚔️</span>
          魔王讨伐
        </button>
        <button
          onClick={onOpenSeason}
          className="btn-game hud-panel rounded-lg py-2.5 text-sm font-bold transition-all hover:border-wc-primary/50 flex items-center justify-center gap-1.5"
          style={{ borderColor: 'rgba(60, 60, 100, 0.8)' }}
        >
          <span className="text-base leading-none">🏁</span>
          本周赛道
        </button>
        <button
          onClick={onOpenAlbum}
          className="btn-game hud-panel rounded-lg py-2.5 text-sm font-bold transition-all hover:border-wc-primary/50 flex items-center justify-center gap-1.5 relative"
          style={{ borderColor: 'rgba(60, 60, 100, 0.8)' }}
        >
          <img src="/assets/blocks/block_limited.png" alt="" className="w-4 h-4 object-contain" />
          水晶图鉴
          {(stats?.draw_tickets ?? 0) > 0 && (
            <span className="absolute -top-1 -right-1 px-1.5 py-0.5 text-[10px] rounded-full bg-wc-gold text-wc-bg font-bold font-game-mono animate-bounce">
              {stats?.draw_tickets}
            </span>
          )}
        </button>
      </div>

      {pickingDrill && (
        <div
          className="fixed inset-0 bg-black/80 flex items-center justify-center z-50 p-4"
          onClick={() => setPickingDrill(false)}
        >
          <div
            className="bg-wc-surface border border-wc-border rounded-2xl p-5 max-w-sm w-full pop-in"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className="text-lg font-bold font-game text-center mb-1">自由探险</h3>
            <p className="text-xs text-wc-text-muted text-center mb-4">
              练多少都不影响连续天数，也不占用今天的时段
            </p>

            <div className="space-y-2">
              {DRILL_OPTIONS.map((opt) => {
                const disabled = opt.mode === 'dictation' && !audioOn
                return (
                  <button
                    key={opt.label}
                    disabled={disabled}
                    onClick={() => {
                      setPickingDrill(false)
                      onStartTraining('free', opt.mode)
                    }}
                    className={`w-full rounded-xl p-3 text-left border transition flex items-center gap-3 ${
                      disabled
                        ? 'border-wc-border/50 bg-wc-surface/50 opacity-50 cursor-default'
                        : 'border-wc-border bg-wc-surface-2 hover:border-wc-primary cursor-pointer'
                    }`}
                  >
                    <span className="text-2xl">{opt.icon}</span>
                    <span className="flex-1 min-w-0">
                      <span className="block text-sm font-bold">{opt.label}</span>
                      <span className="block text-xs text-wc-text-muted">
                        {/* 不可选时说明原因，而不是让按钮无声变灰 */}
                        {disabled ? '需要先在设置里开启发音' : opt.hint}
                      </span>
                    </span>
                  </button>
                )
              })}
            </div>

            <button
              onClick={() => setPickingDrill(false)}
              className="w-full mt-3 py-2 text-sm text-wc-text-muted hover:text-wc-text transition"
            >
              取消
            </button>
          </div>
        </div>
      )}
    </div>
  )
}
