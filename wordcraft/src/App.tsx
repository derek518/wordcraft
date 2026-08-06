import { useState, useEffect, useCallback } from 'react'
import AdventureMap from './components/AdventureMap'
import WordTrainer from './components/WordTrainer'
import StatsPanel from './components/StatsPanel'
import * as api from './data/api'
import { levelProgress } from './core/progression'
import type { OverallStats, SessionType } from './core/types'
import './index.css'

type View = 'map' | 'train' | 'stats'

export default function App() {
  const [view, setView] = useState<View>('map')
  const [sessionType, setSessionType] = useState<SessionType>('morning')
  const [stats, setStats] = useState<OverallStats | null>(null)
  const [showWelcome, setShowWelcome] = useState(false)
  const [bootError, setBootError] = useState('')

  const loadStats = useCallback(async () => {
    try {
      setStats(await api.getOverallStats())
    } catch (e) {
      setBootError(e instanceof Error ? e.message : String(e))
    }
  }, [])

  /**
   * 首次启动：导入内置词库。
   *
   * TODO(T18): `src/data/words.ts` 是 52 词的占位词库（见 MOCKS.md M8），
   * 由真实的人教版 + 外研版融合词库替换。
   */
  const bootstrap = useCallback(async () => {
    try {
      const done = await api.getSetting('onboarding_done')
      if (done === 'true') return

      const { newbieZoneWords } = await import('./data/words')
      const payload = newbieZoneWords.map((w) => ({
        word: w.word,
        phonetic: w.phonetic,
        pos: w.pos,
        meaning: w.meaning,
        example_1: w.example_1,
        example_2: w.example_2,
        level: w.level,
        frequency_band: w.frequency_band,
        zone: 'newbie',
        source_edition: '',
      }))

      const outcome = await api.importWords(payload)
      if (outcome.rejected.length > 0) {
        // 静默跳过会让某些词永远不出现，且无从察觉
        console.warn('部分词条未通过校验：', outcome.rejected)
      }

      await api.setSetting('onboarding_done', 'true')
      setShowWelcome(true)
    } catch (e) {
      setBootError(e instanceof Error ? e.message : String(e))
    }
  }, [])

  useEffect(() => {
    void (async () => {
      await bootstrap()
      await loadStats()
    })()
  }, [bootstrap, loadStats])

  const startTraining = (type: SessionType) => {
    setSessionType(type)
    setView('train')
  }

  const finishTraining = () => {
    setView('map')
    void loadStats()
  }

  const progress = stats ? levelProgress(stats.total_xp) : null

  return (
    <div className="min-h-screen bg-wc-bg text-wc-text">
      <header className="flex items-center justify-between px-6 py-3 bg-wc-surface border-b border-wc-border">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 rounded bg-gradient-to-br from-wc-primary to-wc-accent flex items-center justify-center text-sm font-bold">
            WC
          </div>
          <h1 className="text-lg font-bold tracking-wide">WordCraft</h1>
        </div>

        {stats && progress && (
          <div className="flex items-center gap-4 text-sm">
            <div className="flex items-center gap-1.5">
              <span className="text-wc-gold">⭐</span>
              <span className="font-mono">Lv.{progress.level}</span>
            </div>
            <div className="flex items-center gap-1.5">
              <span className="text-wc-accent">💎</span>
              <span className="font-mono">{stats.total_xp} XP</span>
            </div>
            <div className="flex items-center gap-1.5">
              <span className="text-wc-fire">🔥</span>
              <span className="font-mono">{stats.current_streak} 天</span>
            </div>
          </div>
        )}
      </header>

      {bootError && (
        <div className="mx-4 mt-4 p-3 rounded-lg bg-wc-danger/10 border border-wc-danger/30 text-sm">
          <span className="font-bold text-wc-danger">数据加载失败：</span>
          <span className="text-wc-text-muted ml-1 break-words">{bootError}</span>
        </div>
      )}

      <main className="p-4">
        {view === 'map' && (
          <AdventureMap
            onStartTraining={startTraining}
            onOpenStats={() => setView('stats')}
            stats={stats}
          />
        )}
        {view === 'train' && <WordTrainer sessionType={sessionType} onFinish={finishTraining} />}
        {view === 'stats' && <StatsPanel onBack={() => setView('map')} />}
      </main>

      {showWelcome && (
        <div className="fixed inset-0 bg-black/70 flex items-center justify-center z-50">
          <div className="bg-wc-surface border border-wc-border rounded-xl p-8 max-w-md w-full mx-4 pop-in">
            <h2 className="text-2xl font-bold mb-4 text-center">🎮 欢迎来到遗忘之境</h2>
            <p className="text-wc-text-muted mb-6 text-center leading-relaxed">
              你是一位掉入遗忘之境的冒险者。
              <br />
              收集词汇水晶，击败遗忘魔王，
              <br />
              建造属于你的家园！
            </p>
            <div className="space-y-3 mb-6">
              <div className="flex items-center gap-3 text-sm">
                <span className="text-wc-success text-lg">✨</span>
                <span>每天早中晚三个传送门自动开启</span>
              </div>
              <div className="flex items-center gap-3 text-sm">
                <span className="text-wc-accent text-lg">💎</span>
                <span>答对越快，水晶越亮</span>
              </div>
              <div className="flex items-center gap-3 text-sm">
                <span className="text-wc-gold text-lg">🏠</span>
                <span>收集的水晶可以用来建造家园</span>
              </div>
            </div>
            <button
              onClick={() => setShowWelcome(false)}
              className="w-full py-3 bg-gradient-to-r from-wc-primary to-wc-primary-bright rounded-lg font-bold hover:opacity-90 transition"
            >
              开始冒险！
            </button>
          </div>
        </div>
      )}
    </div>
  )
}
