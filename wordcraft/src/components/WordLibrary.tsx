import { useState, useEffect, useCallback } from 'react'
import * as api from '../data/api'

interface WordLibraryProps {
  onBack: () => void
}

const ZONE_NAMES: Record<string, string> = {
  newbie: '新手村',
  grass: '清风平原',
  water: '蓝水湖泊',
  fire: '赤焰山脉',
  thunder: '雷霆峡谷',
  ice: '永冬之巅',
  rock: '磐石秘境',
}

const ZONE_COLORS: Record<string, string> = {
  newbie: '#e2e8f0',
  grass: '#4ade80',
  water: '#3b82f6',
  fire: '#ef4444',
  thunder: '#a855f7',
  ice: '#67e8f9',
  rock: '#f59e0b',
}

/**
 * 词库浏览。spec §4.2 F8「水晶图谱，按区域/元素筛选」。
 *
 * 3657 词无法一次呈现，也不该无限滚动——目标用户面对长列表容易失焦。
 * 因此以搜索为主、区域筛选为辅，每次只给一屏。
 */
export default function WordLibrary({ onBack }: WordLibraryProps) {
  const [keyword, setKeyword] = useState('')
  const [zone, setZone] = useState<string | null>(null)
  const [results, setResults] = useState<api.LibraryWord[]>([])
  const [zones, setZones] = useState<api.ZoneProgress[]>([])
  const [total, setTotal] = useState(0)
  const [error, setError] = useState('')
  const [searching, setSearching] = useState(false)
  const [detail, setDetail] = useState<api.LibraryWord | null>(null)

  useEffect(() => {
    void (async () => {
      try {
        const [z, stats] = await Promise.all([api.getZoneProgress(), api.getOverallStats()])
        setZones(z)
        setTotal(stats.total_words)
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e))
      }
    })()
  }, [])

  const search = useCallback(async (kw: string) => {
    if (!kw.trim()) {
      setResults([])
      return
    }
    setSearching(true)
    setError('')
    try {
      setResults(await api.searchWords(kw.trim(), 60))
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setSearching(false)
    }
  }, [])

  // 输入停顿 300ms 后才查。每敲一个字母就发一次请求，
  // 既浪费也会让结果闪烁
  useEffect(() => {
    const timer = setTimeout(() => void search(keyword), 300)
    return () => clearTimeout(timer)
  }, [keyword, search])

  const shown = zone ? results.filter((w) => w.zone === zone) : results

  return (
    <div className="max-w-2xl mx-auto">
      <div className="flex items-center justify-between mb-5">
        <button
          onClick={onBack}
          className="flex items-center gap-1 text-sm text-wc-text-muted hover:text-wc-text transition"
        >
          <span className="text-lg">←</span> 返回
        </button>
        <h2 className="text-xl font-bold font-game">🔍 水晶图谱</h2>
        <span className="text-sm font-game-mono text-wc-text-muted">
          {shown.length > 0 ? `${shown.length} 条` : ''}
        </span>
      </div>

      <input
        type="text"
        value={keyword}
        onChange={(e) => setKeyword(e.target.value)}
        placeholder="搜索单词或释义…"
        autoFocus
        className="w-full px-4 py-3 rounded-xl border border-wc-border bg-wc-surface-2 outline-none focus:border-wc-primary transition mb-3"
      />

      {/* 区域筛选。词数取自后端，与地图页显示的一致 */}
      <div className="flex gap-2 overflow-x-auto pb-1 mb-4">
        <button
          onClick={() => setZone(null)}
          className={`px-3 py-1.5 rounded-lg text-xs whitespace-nowrap border transition ${
            zone === null
              ? 'border-wc-primary bg-wc-primary/15'
              : 'border-wc-border bg-wc-surface-2 hover:border-wc-primary/50'
          }`}
        >
          全部
        </button>
        {zones.map((z) => (
          <button
            key={z.key}
            onClick={() => setZone(zone === z.key ? null : z.key)}
            className={`px-3 py-1.5 rounded-lg text-xs whitespace-nowrap border transition flex items-center gap-1.5 ${
              zone === z.key
                ? 'border-wc-primary bg-wc-primary/15'
                : 'border-wc-border bg-wc-surface-2 hover:border-wc-primary/50'
            }`}
          >
            <span
              className="w-2 h-2 rounded-full"
              style={{ backgroundColor: ZONE_COLORS[z.key] }}
            />
            {z.name}
            <span className="font-game-mono text-wc-text-muted">{z.total}</span>
          </button>
        ))}
      </div>

      {error && (
        <div className="p-3 rounded-xl bg-wc-danger/10 border border-wc-danger/30 text-sm mb-4">
          <span className="font-bold text-wc-danger">请求失败：</span>
          <span className="text-wc-text-muted ml-1 break-words">{error}</span>
        </div>
      )}

      {!keyword.trim() ? (
        <div className="hud-panel rounded-2xl p-8 text-center">
          <div className="text-4xl mb-3">📖</div>
          <p className="text-sm text-wc-text-muted">
            输入单词或中文释义开始查找
            <br />
            {/* 总数取自后端。写死的数字会在词库更新后悄悄变成谎话——
                蓝图描述与赛道积分都栽在同一件事上 */}
            <span className="text-xs">
              词库共 {total.toLocaleString()} 个高考考纲词
            </span>
          </p>
        </div>
      ) : searching ? (
        <div className="text-center py-8 text-wc-text-muted text-sm animate-pulse">
          查找中…
        </div>
      ) : shown.length === 0 ? (
        <div className="hud-panel rounded-2xl p-8 text-center text-sm text-wc-text-muted">
          没有匹配的词
          {zone && <>（已按「{ZONE_NAMES[zone]}」筛选）</>}
        </div>
      ) : (
        <div className="space-y-2">
          {shown.map((w) => (
            <button
              key={w.id}
              onClick={() => setDetail(w)}
              className="w-full hud-panel rounded-xl p-3 text-left hover:border-wc-primary/50 transition flex items-center gap-3"
            >
              <span
                className="w-2 h-8 rounded-full flex-shrink-0"
                style={{ backgroundColor: ZONE_COLORS[w.zone] ?? '#475569' }}
              />
              <div className="flex-1 min-w-0">
                <div className="flex items-baseline gap-2">
                  <span className="font-bold">{w.word}</span>
                  <span className="text-xs text-wc-accent">{w.phonetic}</span>
                </div>
                <div className="text-sm text-wc-text-muted truncate">
                  {w.pos} {w.meaning}
                </div>
              </div>
              <span className="text-[10px] font-game-mono text-wc-text-muted flex-shrink-0">
                B{w.frequency_band}
              </span>
            </button>
          ))}
        </div>
      )}

      {detail && (
        <div
          className="fixed inset-0 bg-black/80 flex items-center justify-center z-50 p-4"
          onClick={() => setDetail(null)}
        >
          <div
            className="bg-wc-surface border border-wc-border rounded-xl p-6 max-w-md w-full pop-in"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="text-center mb-4">
              <div className="text-3xl font-bold mb-1">{detail.word}</div>
              <div className="text-sm text-wc-accent">{detail.phonetic}</div>
            </div>
            <div className="text-sm mb-4">
              <span className="text-wc-text-muted">{detail.pos}</span>{' '}
              <span className="font-bold">{detail.meaning}</span>
            </div>
            <div className="text-sm text-wc-text-muted bg-wc-bg rounded-lg p-3 space-y-1 mb-4">
              <div>📝 {detail.example_1}</div>
              {detail.example_2 && <div>📝 {detail.example_2}</div>}
            </div>
            <div className="flex items-center justify-between text-xs text-wc-text-muted">
              <span>
                {ZONE_NAMES[detail.zone] ?? detail.zone} · 频段 {detail.frequency_band}
              </span>
              <button
                onClick={() => setDetail(null)}
                className="px-4 py-1.5 bg-wc-surface-2 border border-wc-border rounded-lg hover:border-wc-primary transition"
              >
                关闭
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
