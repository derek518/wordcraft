import { useState, useEffect, useCallback } from 'react'
import * as api from '../data/api'
import { playCorrect, playLevelUp } from '../core/sound'

interface CardAlbumProps {
  onBack: () => void
}

const RARITY_STYLE: Record<number, { label: string; ring: string; glow: string; crystal: string }> = {
  1: { label: '普通', ring: 'border-slate-500/40', glow: 'rarity-glow-1', crystal: '/assets/crystals/crystal_rock_dim.png' },
  2: { label: '稀有', ring: 'border-wc-accent/60', glow: 'rarity-glow-2', crystal: '/assets/crystals/crystal_water_bright.png' },
  3: { label: '传说', ring: 'border-wc-gold', glow: 'rarity-glow-3', crystal: '/assets/crystals/crystal_fire_bright.png' },
}

export default function CardAlbum({ onBack }: CardAlbumProps) {
  const [entries, setEntries] = useState<api.CollectionEntry[]>([])
  const [tickets, setTickets] = useState(0)
  const [error, setError] = useState('')
  const [drawing, setDrawing] = useState(false)
  const [revealed, setRevealed] = useState<api.DrawResult | null>(null)
  const [detail, setDetail] = useState<api.CollectionEntry | null>(null)
  const [showFlash, setShowFlash] = useState(false)

  const load = useCallback(async () => {
    setError('')
    try {
      const [list, stats] = await Promise.all([api.getCollection(), api.getOverallStats()])
      setEntries(list)
      setTickets(stats.draw_tickets)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    }
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  const draw = async () => {
    if (drawing || tickets <= 0) return
    setDrawing(true)
    try {
      const result = await api.drawCard()
      if (result.card.rarity >= 3) {
        playLevelUp()
        setShowFlash(true)
        setTimeout(() => setShowFlash(false), 800)
      } else {
        playCorrect(0)
      }

      setRevealed(result)
      setTickets(result.tickets_left)
      await load()
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setDrawing(false)
    }
  }

  const closeReveal = async () => {
    const id = revealed?.card.id
    setRevealed(null)
    if (id !== undefined) {
      try {
        await api.markCardsSeen([id])
        await load()
      } catch {
        // ignore
      }
    }
  }

  const collected = entries.filter((e) => e.count > 0).length
  const total = entries.length
  const progressPct = total > 0 ? (collected / total) * 100 : 0

  return (
    <div className="max-w-2xl mx-auto">
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <button
          onClick={onBack}
          className="flex items-center gap-1 text-sm text-wc-text-muted hover:text-wc-text transition"
        >
          <span className="text-lg">←</span> 返回
        </button>
        <div className="flex items-center gap-2">
          <img src="/assets/crystals/crystal_fire_bright.png" alt="" className="w-6 h-6 object-contain" />
          <h2 className="text-xl font-bold font-game">水晶图鉴</h2>
        </div>
        <div className="flex items-center gap-1 text-sm font-game-mono text-wc-text-muted">
          <img src="/assets/effects/star.png" alt="" className="w-4 h-4 object-contain" />
          <span>{collected}/{total}</span>
        </div>
      </div>

      {error && (
        <div className="p-3 rounded-xl bg-wc-danger/10 border border-wc-danger/30 text-sm mb-6 hud-panel">
          <span className="font-bold text-wc-danger">出错了：</span>
          <span className="text-wc-text-muted ml-1 break-words">{error}</span>
          <button onClick={load} className="ml-2 underline hover:text-wc-text">
            重试
          </button>
        </div>
      )}

      {/* Draw Area */}
      <div className="hud-panel rounded-2xl p-5 mb-6 flex items-center justify-between relative overflow-hidden neon-border">
        <div className="absolute top-0 left-0 right-0 h-[1px] bg-gradient-to-r from-transparent via-wc-primary/30 to-transparent" />
        <div>
          <div className="text-sm text-wc-text-muted">抽卡券</div>
          <div className="flex items-center gap-2 mt-1">
            <img src="/assets/effects/star.png" alt="" className="w-8 h-8 object-contain" />
            <span className="text-3xl font-bold text-wc-gold font-game-mono">{tickets}</span>
          </div>
          <div className="text-xs text-wc-text-muted mt-1">完成一个传送门 +1，完美日额外 +1</div>
        </div>
        <button
          onClick={draw}
          disabled={tickets <= 0 || drawing}
          className="px-6 py-3 btn-game bg-gradient-to-r from-wc-primary to-wc-primary-bright rounded-xl font-bold hover:opacity-90 transition disabled:opacity-40 disabled:cursor-not-allowed relative"
          style={{ boxShadow: tickets > 0 ? '0 0 20px rgba(124, 58, 237, 0.4)' : 'none' }}
        >
          {drawing ? (
            <span className="flex items-center gap-2">
              <span className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
              开启中…
            </span>
          ) : (
            '抽一张'
          )}
        </button>
      </div>

      {/* Collection Progress */}
      <div className="mb-4">
        <div className="flex items-center justify-between text-xs text-wc-text-muted mb-1">
          <span>收集进度</span>
          <span className="font-game-mono">{progressPct.toFixed(0)}%</span>
        </div>
        <div className="h-1.5 bg-wc-surface-2 rounded-full overflow-hidden">
          <div
            className="h-full progress-shine rounded-full transition-all duration-700"
            style={{ width: `${progressPct}%` }}
          />
        </div>
      </div>

      {/* Grid */}
      <div className="grid grid-cols-4 sm:grid-cols-5 gap-3">
        {entries.map((entry) => {
          const owned = entry.count > 0
          const style = RARITY_STYLE[entry.card.rarity] ?? RARITY_STYLE[1]
          return (
            <button
              key={entry.card.id}
              onClick={() => owned && setDetail(entry)}
              disabled={!owned}
              className={`relative aspect-square rounded-xl border-2 p-2 transition-all ${style.ring} ${
                owned
                  ? `bg-wc-surface hover:scale-105 cursor-pointer ${style.glow}`
                  : 'bg-wc-surface-2 cursor-default'
              }`}
            >
              <img
                src={entry.card.image_path}
                alt={owned ? entry.card.name : '未收集'}
                className={`w-full h-full object-contain ${
                  owned ? '' : 'brightness-0 opacity-30'
                }`}
              />
              {entry.is_new && owned && (
                <span className="absolute top-1 right-1 w-2.5 h-2.5 rounded-full bg-wc-danger animate-pulse" />
              )}
              {entry.count > 1 && (
                <span className="absolute bottom-1 right-1 text-xs font-game-mono text-wc-text-muted">
                  ×{entry.count}
                </span>
              )}
              {!owned && (
                <img
                  src="/assets/blocks/block_normal.png"
                  alt=""
                  className="absolute inset-0 w-4 h-4 m-auto opacity-20 object-contain"
                />
              )}
            </button>
          )
        })}
      </div>

      {/* Reveal Modal */}
      {revealed && (
        <div
          className="fixed inset-0 bg-black/85 flex items-center justify-center z-50"
          onClick={closeReveal}
        >
          {/* Flash effect */}
          {showFlash && (
            <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
              <div className="w-96 h-96 rounded-full bg-wc-gold/20 blur-3xl reveal-flash" />
            </div>
          )}

          <div className="text-center relative">
            <div className="text-sm text-wc-gold mb-3 font-game">
              {revealed.is_first ? '✨ 新卡牌！' : `已有 ×${revealed.count}`}
            </div>

            <div
              className={`w-56 h-56 mx-auto rounded-2xl border-4 bg-wc-surface p-6 card-reveal relative ${
                (RARITY_STYLE[revealed.card.rarity] ?? RARITY_STYLE[1]).ring
              } ${(RARITY_STYLE[revealed.card.rarity] ?? RARITY_STYLE[1]).glow}`}
            >
              <div className="absolute top-0 left-0 right-0 h-[1px] bg-gradient-to-r from-transparent via-wc-primary/30 to-transparent" />
              <img
                src={revealed.card.image_path}
                alt={revealed.card.name}
                className="w-full h-full object-contain"
              />
            </div>

            <div className="mt-5 text-2xl font-bold font-game">{revealed.card.name}</div>
            <div className="flex items-center justify-center gap-1 text-sm text-wc-accent mt-1">
              <img
                src={(RARITY_STYLE[revealed.card.rarity] ?? RARITY_STYLE[1]).crystal}
                alt=""
                className="w-4 h-4 object-contain"
              />
              <span>{(RARITY_STYLE[revealed.card.rarity] ?? RARITY_STYLE[1]).label}</span>
            </div>
            <p className="text-sm text-wc-text-muted mt-3 max-w-xs mx-auto">
              {revealed.card.trivia}
            </p>
            <div className="text-xs text-wc-text-muted mt-5 animate-pulse">点击任意处继续</div>
          </div>
        </div>
      )}

      {/* Detail Modal */}
      {detail && (
        <div
          className="fixed inset-0 bg-black/80 flex items-center justify-center z-50 p-4"
          onClick={() => setDetail(null)}
        >
          <div
            className="hud-panel rounded-2xl p-6 max-w-sm w-full text-center pop-in-bounce relative"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="absolute top-0 left-0 right-0 h-[1px] bg-gradient-to-r from-transparent via-wc-primary/30 to-transparent" />

            <div
              className={`w-40 h-40 mx-auto rounded-xl border-2 p-4 mb-4 ${
                (RARITY_STYLE[detail.card.rarity] ?? RARITY_STYLE[1]).ring
              } ${(RARITY_STYLE[detail.card.rarity] ?? RARITY_STYLE[1]).glow}`}
            >
              <img
                src={detail.card.image_path}
                alt={detail.card.name}
                className="w-full h-full object-contain"
              />
            </div>

            <h3 className="text-xl font-bold font-game">{detail.card.name}</h3>
            <div className="flex items-center justify-center gap-1 text-sm text-wc-accent mb-3 mt-1">
              <img
                src={(RARITY_STYLE[detail.card.rarity] ?? RARITY_STYLE[1]).crystal}
                alt=""
                className="w-4 h-4 object-contain"
              />
              <span>{(RARITY_STYLE[detail.card.rarity] ?? RARITY_STYLE[1]).label} · 持有 ×{detail.count}</span>
            </div>
            <p className="text-sm text-wc-text-muted mb-4">{detail.card.trivia}</p>
            <div className="text-xs text-wc-text-muted/60 break-words">{detail.card.source}</div>
            <button
              onClick={() => setDetail(null)}
              className="mt-5 px-6 py-2 btn-game bg-wc-surface-2 border border-wc-border rounded-xl text-sm hover:border-wc-primary transition"
            >
              关闭
            </button>
          </div>
        </div>
      )}
    </div>
  )
}
