import { useState, useEffect, useCallback } from 'react'
import * as api from '../data/api'
import { playCorrect, playLevelUp } from '../core/sound'

interface CardAlbumProps {
  onBack: () => void
}

type FilterRarity = 'all' | '1' | '2' | '3'
type FilterOwned = 'all' | 'owned' | 'missing'
type ViewMode = 'grid' | 'element'

const RARITY_META: Record<number, { label: string; color: string; glow: string; back: string }> = {
  1: { label: '普通', color: 'text-slate-400', glow: 'rarity-glow-1', back: '/assets/cards/back/back_common.png' },
  2: { label: '稀有', color: 'text-wc-accent', glow: 'rarity-glow-2', back: '/assets/cards/back/back_rare.png' },
  3: { label: '传说', color: 'text-wc-gold', glow: 'rarity-glow-3', back: '/assets/cards/back/back_legend.png' },
}

const ELEMENT_META: Record<string, { name: string; color: string; icon: string }> = {
  grass: { name: '草', color: '#22c55e', icon: '🌿' },
  water: { name: '水', color: '#3b82f6', icon: '💧' },
  fire: { name: '火', color: '#ef4444', icon: '🔥' },
  thunder: { name: '雷', color: '#a855f7', icon: '⚡' },
  ice: { name: '冰', color: '#22d3ee', icon: '❄️' },
  rock: { name: '岩', color: '#f59e0b', icon: '🪨' },
}

function getElementFromPath(path: string): string {
  const m = path.match(/cards\/(common|rare|legend)\/(\w+)_/)
  return m ? m[2] : 'unknown'
}

export default function CardAlbum({ onBack }: CardAlbumProps) {
  const [entries, setEntries] = useState<api.CollectionEntry[]>([])
  const [tickets, setTickets] = useState(0)
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)

  const [filterRarity, setFilterRarity] = useState<FilterRarity>('all')
  const [filterOwned, setFilterOwned] = useState<FilterOwned>('all')
  const [viewMode, setViewMode] = useState<ViewMode>('grid')

  const [multiResults, setMultiResults] = useState<api.DrawResult[] | null>(null)
  const [revealIndex, setRevealIndex] = useState(-1)
  const [isRevealing, setIsRevealing] = useState(false)
  const [legendFlash, setLegendFlash] = useState(false)

  const [detail, setDetail] = useState<api.CollectionEntry | null>(null)

  const load = useCallback(async () => {
    setError('')
    try {
      const [list, stats] = await Promise.all([api.getCollection(), api.getOverallStats()])
      setEntries(list)
      setTickets(stats.draw_tickets)
    } catch (e) {
      // 后端失败一律显示错误态。此处曾在 dev 下降级到本地假卡池——
      // 迁移 010 崩溃那次，若不是应用整个起不来，看到的会是一屏漂亮的假卡，
      // 真正的故障被完整盖住
      setError(e instanceof Error ? e.message : String(e))
    }
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  const drawOne = async () => {
    if (loading || tickets <= 0) return
    setLoading(true)
    try {
      const result = await api.drawCard()
      if (result.card.rarity >= 3) playLevelUp()
      else playCorrect(0)
      setTickets(result.tickets_left)
      await load()
      const entry = entries.find(e => e.card.id === result.card.id)
      if (entry) setDetail(entry)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }

  const drawTen = async () => {
    if (loading || tickets < 10) return
    setLoading(true)
    setError('')
    try {
      const results: api.DrawResult[] = []
      for (let i = 0; i < 10; i++) {
        const r = await api.drawCard()
        results.push(r)
      }
      const hasLegend = results.some(r => r.card.rarity >= 3)
      if (hasLegend) {
        playLevelUp()
        setLegendFlash(true)
        setTimeout(() => setLegendFlash(false), 1500)
      } else {
        playCorrect(0)
      }
      setTickets(prev => prev - 10)
      setMultiResults(results)
      setRevealIndex(-1)
      setIsRevealing(true)
      await load()
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    if (!isRevealing || !multiResults) return
    if (revealIndex < multiResults.length - 1) {
      const timer = setTimeout(() => {
        setRevealIndex(prev => prev + 1)
      }, 600)
      return () => clearTimeout(timer)
    }
  }, [isRevealing, revealIndex, multiResults])

  const closeMulti = async () => {
    if (multiResults) {
      const newIds = multiResults.filter(r => r.is_first).map(r => r.card.id)
      if (newIds.length > 0) {
        try { await api.markCardsSeen(newIds) } catch { /* ignore */ }
      }
    }
    setMultiResults(null)
    setRevealIndex(-1)
    setIsRevealing(false)
    await load()
  }

  const filtered = entries.filter(e => {
    if (filterRarity !== 'all' && e.card.rarity !== parseInt(filterRarity)) return false
    if (filterOwned === 'owned' && e.count === 0) return false
    if (filterOwned === 'missing' && e.count > 0) return false
    return true
  })

  const collected = entries.filter(e => e.count > 0).length
  const total = entries.length
  const progressPct = total > 0 ? (collected / total) * 100 : 0

  const elementGroups = viewMode === 'element'
    ? Object.entries(ELEMENT_META).map(([key, meta]) => ({
        key,
        meta,
        cards: filtered.filter(e => getElementFromPath(e.card.image_path) === key),
      })).filter(g => g.cards.length > 0)
    : []

  return (
    <div className="max-w-4xl mx-auto">
      <div className="flex items-center justify-between mb-3">
        <button onClick={onBack} className="flex items-center gap-1 text-sm text-wc-text-dim hover:text-wc-text transition">
          <span className="text-lg">←</span> 返回
        </button>
        <div className="flex items-center gap-2">
          <img src="/assets/crystals/crystal_fire_bright.png" alt="" className="w-6 h-6 object-contain" />
          <h2 className="text-xl font-bold font-game">水晶图鉴</h2>
        </div>
        <div className="flex items-center gap-1 text-sm font-game-mono text-wc-text-dim">
          <img src="/assets/effects/star.png" alt="" className="w-4 h-4 object-contain" />
          <span>{collected}/{total}</span>
        </div>
      </div>

      {error && (
        <div className="p-3 rounded-xl bg-wc-danger/10 border border-wc-danger/30 text-sm mb-3 hud-panel">
          <span className="font-bold text-wc-danger">出错了：</span>
          <span className="text-wc-text-dim ml-1 break-words">{error}</span>
          <button onClick={load} className="ml-2 underline hover:text-wc-text">重试</button>
        </div>
      )}


      <div className="hud-panel rounded-2xl p-3 mb-3 flex items-center justify-between relative overflow-hidden">
        <div className="absolute top-0 left-0 right-0 h-[1px] bg-gradient-to-r from-transparent via-wc-primary/30 to-transparent" />
        <div>
          <div className="text-sm text-wc-text-dim">抽卡券</div>
          <div className="flex items-center gap-2 mt-1">
            <img src="/assets/effects/star.png" alt="" className="w-7 h-7 object-contain" />
            <span className="text-2xl font-bold text-wc-gold font-game-mono">{tickets}</span>
          </div>
        </div>
        <div className="flex gap-2">
          <button
            onClick={drawOne}
            disabled={tickets <= 0 || loading}
            className="px-4 py-2 btn-game bg-wc-surface-2 border border-wc-border rounded-xl font-bold text-sm transition disabled:opacity-40"
          >
            {loading ? '...' : '抽一张'}
          </button>
          <button
            onClick={drawTen}
            disabled={tickets < 10 || loading}
            className="px-5 py-2 btn-game bg-gradient-to-r from-wc-primary to-wc-primary-bright rounded-xl font-bold text-sm transition disabled:opacity-40"
            style={{ boxShadow: tickets >= 10 ? '0 0 15px rgba(124,58,237,0.4)' : 'none' }}
          >
            {loading ? '开启中...' : '十连抽 ✦'}
          </button>
        </div>
      </div>

      <div className="mb-2">
        <div className="flex items-center justify-between text-xs text-wc-text-dim mb-1">
          <span>收集进度</span>
          <span className="font-game-mono">{progressPct.toFixed(1)}%</span>
        </div>
        <div className="h-2 bg-wc-surface-2 rounded-full overflow-hidden">
          <div className="h-full progress-shine rounded-full transition-all duration-700" style={{ width: `${progressPct}%` }} />
        </div>
        <div className="flex gap-3 mt-1.5 text-xs text-wc-text-dim">
          {[1, 2, 3].map(r => {
            const count = entries.filter(e => e.card.rarity === r && e.count > 0).length
            const totalR = entries.filter(e => e.card.rarity === r).length
            const meta = RARITY_META[r]
            return (
              <span key={r} className={meta.color}>
                {meta.label} {count}/{totalR}
              </span>
            )
          })}
        </div>
      </div>

      <div className="flex flex-wrap gap-2 mb-3">
        <div className="flex gap-1 bg-wc-surface-2 rounded-lg p-1">
          {([['all', '全部'], ['1', '普通'], ['2', '稀有'], ['3', '传说']] as [FilterRarity, string][]).map(([v, l]) => (
            <button
              key={v}
              onClick={() => setFilterRarity(v as FilterRarity)}
              className={`px-2.5 py-1 rounded-md text-xs font-medium transition ${
                filterRarity === v ? 'bg-wc-primary/25 text-wc-primary-bright' : 'text-wc-text-dim hover:text-wc-text'
              }`}
            >
              {l}
            </button>
          ))}
        </div>
        <div className="flex gap-1 bg-wc-surface-2 rounded-lg p-1">
          {([['all', '全部'], ['owned', '已收集'], ['missing', '未收集']] as [FilterOwned, string][]).map(([v, l]) => (
            <button
              key={v}
              onClick={() => setFilterOwned(v as FilterOwned)}
              className={`px-2.5 py-1 rounded-md text-xs font-medium transition ${
                filterOwned === v ? 'bg-wc-primary/25 text-wc-primary-bright' : 'text-wc-text-dim hover:text-wc-text'
              }`}
            >
              {l}
            </button>
          ))}
        </div>
        <div className="flex gap-1 bg-wc-surface-2 rounded-lg p-1 ml-auto">
          {([['grid', '网格'], ['element', '元素']] as [ViewMode, string][]).map(([v, l]) => (
            <button
              key={v}
              onClick={() => setViewMode(v as ViewMode)}
              className={`px-2.5 py-1 rounded-md text-xs font-medium transition ${
                viewMode === v ? 'bg-wc-primary/25 text-wc-primary-bright' : 'text-wc-text-dim hover:text-wc-text'
              }`}
            >
              {l}
            </button>
          ))}
        </div>
      </div>

      {viewMode === 'grid' ? (
        <div className="grid grid-cols-7 gap-1.5">
          {filtered.map(entry => (
            <CardItem key={entry.card.id} entry={entry} onClick={() => entry.count > 0 && setDetail(entry)} />
          ))}
        </div>
      ) : (
        <div className="space-y-3">
          {elementGroups.map(group => (
            <div key={group.key}>
              <div className="flex items-center gap-2 mb-1.5">
                <span className="text-base">{group.meta.icon}</span>
                <span className="text-sm font-bold" style={{ color: group.meta.color }}>{group.meta.name}系</span>
                <span className="text-xs text-wc-text-dim">{group.cards.filter(c => c.count > 0).length}/{group.cards.length}</span>
              </div>
              <div className="grid grid-cols-7 gap-1.5">
                {group.cards.map(entry => (
                  <CardItem key={entry.card.id} entry={entry} onClick={() => entry.count > 0 && setDetail(entry)} />
                ))}
              </div>
            </div>
          ))}
        </div>
      )}

      {multiResults && (
        <div className="fixed inset-0 bg-black/90 flex items-center justify-center z-50 p-4" onClick={!isRevealing ? closeMulti : undefined}>
          {legendFlash && (
            <div className="absolute inset-0 flex items-center justify-center pointer-events-none">
              <div className="w-[600px] h-[600px] rounded-full bg-wc-gold/15 blur-3xl reveal-flash" />
            </div>
          )}

          <div className="relative max-w-lg w-full">
            <div className="text-center mb-3">
              <div className="text-wc-gold font-game text-lg mb-1">✦ 十连召唤 ✦</div>
              <div className="text-xs text-wc-text-dim">
                {isRevealing && revealIndex < 9 ? `翻开第 ${revealIndex + 2} 张...` : '点击任意处关闭'}
              </div>
            </div>

            <div className="grid grid-cols-5 gap-2">
              {multiResults.map((result, i) => {
                const revealed = i <= revealIndex
                const meta = RARITY_META[result.card.rarity]
                return (
                  <div key={i} className="flip-card aspect-[5/7]" style={{ perspective: '800px' }}>
                    <div
                      className="flip-card-inner relative w-full h-full transition-transform duration-500"
                      style={{
                        transformStyle: 'preserve-3d',
                        transform: revealed ? 'rotateY(180deg)' : 'rotateY(0deg)',
                        transitionDelay: `${i * 50}ms`,
                      }}
                    >
                      <div className="absolute inset-0 backface-hidden" style={{ backfaceVisibility: 'hidden' }}>
                        <img src={meta.back} alt="卡背" className="w-full h-full object-contain rounded-lg" />
                      </div>
                      <div className="absolute inset-0 backface-hidden" style={{ backfaceVisibility: 'hidden', transform: 'rotateY(180deg)' }}>
                        <img src={result.card.image_path} alt={result.card.name} className={`w-full h-full object-contain rounded-lg ${meta.glow}`} />
                        {result.is_first && (
                          <span className="absolute top-0.5 right-0.5 px-1 py-0.5 text-[8px] rounded bg-wc-gold text-wc-bg font-bold">NEW</span>
                        )}
                      </div>
                    </div>
                  </div>
                )
              })}
            </div>

            {!isRevealing && (
              <div className="text-center mt-4">
                <button onClick={closeMulti} className="px-6 py-2 btn-game bg-wc-primary rounded-xl font-bold text-sm">
                  收下卡牌
                </button>
              </div>
            )}
          </div>
        </div>
      )}

      {detail && (
        <div className="fixed inset-0 bg-black/80 flex items-center justify-center z-50 p-4" onClick={() => setDetail(null)}>
          <div className="hud-panel rounded-2xl p-5 max-w-xs w-full pop-in-bounce relative" onClick={e => e.stopPropagation()}>
            <div className="absolute top-0 left-0 right-0 h-[1px] bg-gradient-to-r from-transparent via-wc-primary/30 to-transparent" />
            <div className={`relative rounded-xl overflow-hidden mb-3 ${RARITY_META[detail.card.rarity].glow}`}>
              <img src={detail.card.image_path} alt={detail.card.name} className="w-full aspect-[5/7] object-contain" />
              {detail.count > 1 && (
                <span className="absolute bottom-2 right-2 px-2 py-0.5 text-xs rounded-full bg-wc-bg/80 text-wc-gold font-game-mono">×{detail.count}</span>
              )}
            </div>
            <h3 className="text-lg font-bold font-game text-center">{detail.card.name}</h3>
            <div className="flex items-center justify-center gap-2 mt-1 mb-2">
              <span className={`text-xs ${RARITY_META[detail.card.rarity].color}`}>{RARITY_META[detail.card.rarity].label}</span>
              <span className="text-xs text-wc-text-dim">·</span>
              <span className="text-xs text-wc-text-dim">{detail.card.card_type === 'shard' ? '碎片' : detail.card.card_type === 'creature' ? '生物' : detail.card.card_type === 'item' ? '道具' : detail.card.card_type === 'guardian' ? '守护者' : detail.card.card_type === 'artifact' ? '神器' : detail.card.card_type}</span>
            </div>
            <p className="text-xs text-wc-text-dim text-center leading-relaxed mb-3">{detail.card.trivia}</p>
            <div className="text-[10px] text-wc-text-muted/50 text-center break-words">{detail.card.source}</div>
            <button onClick={() => setDetail(null)} className="mt-3 w-full py-2 btn-game bg-wc-surface-2 border border-wc-border rounded-xl text-sm hover:border-wc-primary transition">关闭</button>
          </div>
        </div>
      )}
    </div>
  )
}

function CardItem({ entry, onClick }: { entry: api.CollectionEntry; onClick: () => void }) {
  const owned = entry.count > 0
  const meta = RARITY_META[entry.card.rarity]
  const element = getElementFromPath(entry.card.image_path)
  const elemColor = ELEMENT_META[element]?.color || '#888'

  return (
    <button
      onClick={onClick}
      disabled={!owned}
      className={`relative aspect-[5/7] rounded-lg overflow-hidden transition-all duration-200 ${
        owned ? 'cursor-pointer hover:scale-105 hover:z-10' : 'cursor-default'
      }`}
      style={{
        boxShadow: owned ? `0 0 8px ${elemColor}30, 0 2px 6px rgba(0,0,0,0.4)` : '0 1px 3px rgba(0,0,0,0.3)',
      }}
    >
      {owned ? (
        <>
          <img src={entry.card.image_path} alt={entry.card.name} className="w-full h-full object-contain" />
          {/* 卡牌名称 - 底部叠加 */}
          <div className="absolute bottom-0 left-0 right-0 bg-gradient-to-t from-black/70 to-transparent pt-3 pb-0.5 px-0.5 text-center pointer-events-none">
            <span className="text-[8px] text-wc-text/90 font-medium truncate block leading-tight">{entry.card.name}</span>
          </div>
          {/* 稀有度角标 */}
          <div className="absolute top-0.5 left-0.5">
            {entry.card.rarity === 3 && <span className="text-[8px] text-wc-gold drop-shadow">✦</span>}
            {entry.card.rarity === 2 && <span className="text-[8px] text-wc-accent drop-shadow">◆</span>}
          </div>
          {/* 新卡标记 */}
          {entry.is_new && (
            <span className="absolute top-0.5 right-0.5 w-2 h-2 rounded-full bg-wc-danger animate-pulse" />
          )}
          {/* 重复数量 */}
          {entry.count > 1 && (
            <span className="absolute bottom-0.5 right-0.5 text-[9px] font-game-mono text-wc-gold bg-wc-bg/70 px-1 rounded">
              ×{entry.count}
            </span>
          )}
        </>
      ) : (
        <>
          <img src={meta.back} alt="未收集" className="w-full h-full object-contain opacity-60" />
          <div className="absolute inset-0 flex items-center justify-center">
            <span className="text-base opacity-30 text-wc-text">?</span>
          </div>
        </>
      )}
    </button>
  )
}
