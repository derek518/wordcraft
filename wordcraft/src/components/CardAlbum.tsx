import { useState, useEffect, useCallback } from 'react'
import * as api from '../data/api'
import { playCorrect, playLevelUp } from '../core/sound'

interface CardAlbumProps {
  onBack: () => void
}

const RARITY_STYLE: Record<number, { label: string; ring: string; glow: string }> = {
  1: { label: '普通', ring: 'border-wc-border', glow: '' },
  2: { label: '稀有', ring: 'border-wc-accent', glow: 'shadow-lg shadow-wc-accent/20' },
  3: { label: '传说', ring: 'border-wc-gold', glow: 'shadow-lg shadow-wc-gold/30' },
}

/**
 * 图鉴与抽卡。contracts §10。
 *
 * 未收集的卡显示剪影而非隐藏——看得见缺口才有收集欲，全部隐藏的话
 * 用户根本不知道自己还差什么。
 */
export default function CardAlbum({ onBack }: CardAlbumProps) {
  const [entries, setEntries] = useState<api.CollectionEntry[]>([])
  const [tickets, setTickets] = useState(0)
  const [error, setError] = useState('')
  const [drawing, setDrawing] = useState(false)
  const [revealed, setRevealed] = useState<api.DrawResult | null>(null)
  const [detail, setDetail] = useState<api.CollectionEntry | null>(null)

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
      // 传说卡用升级音效，普通卡用答对音——听觉上先于视觉给出稀有度暗示
      if (result.card.rarity >= 3) playLevelUp()
      else playCorrect(0)

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
    // 看过即清红点，否则刚抽到的卡在图鉴里还标着「新」
    if (id !== undefined) {
      try {
        await api.markCardsSeen([id])
        await load()
      } catch {
        // 红点没清干净不影响使用，不打断用户
      }
    }
  }

  const collected = entries.filter((e) => e.count > 0).length

  return (
    <div className="max-w-2xl mx-auto">
      <div className="flex items-center justify-between mb-6">
        <button onClick={onBack} className="text-sm text-wc-text-muted hover:text-wc-text transition">
          ← 返回
        </button>
        <h2 className="text-xl font-bold">🎴 水晶图鉴</h2>
        <div className="text-sm font-mono text-wc-text-muted">
          {collected}/{entries.length}
        </div>
      </div>

      {error && (
        <div className="p-3 rounded-lg bg-wc-danger/10 border border-wc-danger/30 text-sm mb-6">
          <span className="font-bold text-wc-danger">出错了：</span>
          <span className="text-wc-text-muted ml-1 break-words">{error}</span>
          <button onClick={load} className="ml-2 underline hover:text-wc-text">
            重试
          </button>
        </div>
      )}

      <div className="bg-gradient-to-br from-wc-primary/20 to-wc-accent/20 border border-wc-primary/30 rounded-xl p-5 mb-6 flex items-center justify-between">
        <div>
          <div className="text-sm text-wc-text-muted">抽卡券</div>
          <div className="text-3xl font-bold text-wc-gold">🎟️ {tickets}</div>
          <div className="text-xs text-wc-text-muted mt-1">完成一个传送门 +1，完美日额外 +1</div>
        </div>
        <button
          onClick={draw}
          disabled={tickets <= 0 || drawing}
          className="px-6 py-3 bg-gradient-to-r from-wc-primary to-wc-primary-bright rounded-lg font-bold hover:opacity-90 transition disabled:opacity-40 disabled:cursor-not-allowed"
        >
          {drawing ? '开启中…' : '抽一张'}
        </button>
      </div>

      <div className="grid grid-cols-4 sm:grid-cols-5 gap-3">
        {entries.map((entry) => {
          const owned = entry.count > 0
          const style = RARITY_STYLE[entry.card.rarity] ?? RARITY_STYLE[1]
          return (
            <button
              key={entry.card.id}
              onClick={() => owned && setDetail(entry)}
              disabled={!owned}
              className={`relative aspect-square rounded-lg border-2 p-2 transition-all ${style.ring} ${
                owned
                  ? `bg-wc-surface hover:scale-105 cursor-pointer ${style.glow}`
                  : 'bg-wc-surface-2 cursor-default'
              }`}
            >
              <img
                src={entry.card.image_path}
                alt={owned ? entry.card.name : '未收集'}
                className={`w-full h-full object-contain ${
                  // 未收集显示纯黑剪影：保留轮廓让人看出这是什么形态，
                  // 又不泄露配色
                  owned ? '' : 'brightness-0 opacity-30'
                }`}
              />
              {entry.is_new && owned && (
                <span className="absolute top-1 right-1 w-2 h-2 rounded-full bg-wc-danger" />
              )}
              {entry.count > 1 && (
                <span className="absolute bottom-1 right-1 text-xs font-mono text-wc-text-muted">
                  ×{entry.count}
                </span>
              )}
            </button>
          )
        })}
      </div>

      {/* 开卡结果 */}
      {revealed && (
        <div
          className="fixed inset-0 bg-black/80 flex items-center justify-center z-50"
          onClick={closeReveal}
        >
          <div className="text-center pop-in">
            <div className="text-sm text-wc-text-muted mb-2">
              {revealed.is_first ? '✨ 新卡牌！' : `已有 ×${revealed.count}`}
            </div>
            <div
              className={`w-56 h-56 mx-auto rounded-xl border-4 bg-wc-surface p-6 ${
                (RARITY_STYLE[revealed.card.rarity] ?? RARITY_STYLE[1]).ring
              } ${(RARITY_STYLE[revealed.card.rarity] ?? RARITY_STYLE[1]).glow}`}
            >
              <img
                src={revealed.card.image_path}
                alt={revealed.card.name}
                className="w-full h-full object-contain"
              />
            </div>
            <div className="mt-4 text-2xl font-bold">{revealed.card.name}</div>
            <div className="text-sm text-wc-accent">
              {(RARITY_STYLE[revealed.card.rarity] ?? RARITY_STYLE[1]).label}
            </div>
            <p className="text-sm text-wc-text-muted mt-3 max-w-xs mx-auto">
              {revealed.card.trivia}
            </p>
            <div className="text-xs text-wc-text-muted mt-4">点击任意处继续</div>
          </div>
        </div>
      )}

      {/* 卡牌详情 */}
      {detail && (
        <div
          className="fixed inset-0 bg-black/80 flex items-center justify-center z-50 p-4"
          onClick={() => setDetail(null)}
        >
          <div
            className="bg-wc-surface border border-wc-border rounded-xl p-6 max-w-sm w-full text-center pop-in"
            onClick={(e) => e.stopPropagation()}
          >
            <img
              src={detail.card.image_path}
              alt={detail.card.name}
              className="w-40 h-40 mx-auto object-contain mb-4"
            />
            <h3 className="text-xl font-bold">{detail.card.name}</h3>
            <div className="text-sm text-wc-accent mb-3">
              {(RARITY_STYLE[detail.card.rarity] ?? RARITY_STYLE[1]).label} · 持有 ×{detail.count}
            </div>
            <p className="text-sm text-wc-text-muted mb-4">{detail.card.trivia}</p>
            {/* spec F12 验收项：素材来源可追溯 */}
            <div className="text-xs text-wc-text-muted/60 break-words">{detail.card.source}</div>
            <button
              onClick={() => setDetail(null)}
              className="mt-5 px-6 py-2 bg-wc-surface-2 border border-wc-border rounded-lg text-sm hover:border-wc-primary transition"
            >
              关闭
            </button>
          </div>
        </div>
      )}
    </div>
  )
}
