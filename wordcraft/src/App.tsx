import { useState, useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import AdventureMap from './components/AdventureMap'
import WordTrainer from './components/WordTrainer'
import StatsPanel from './components/StatsPanel'
import './index.css'

type View = 'map' | 'train' | 'stats' | 'settings'

function App() {
  const [currentView, setCurrentView] = useState<View>('map')
  const [sessionType, setSessionType] = useState<string>('morning')
  const [overallStats, setOverallStats] = useState<any>(null)
  const [isFirstRun, setIsFirstRun] = useState(false)

  useEffect(() => {
    checkFirstRun()
    loadStats()
  }, [])

  const checkFirstRun = async () => {
    try {
      const firstRun = await invoke<string>('get_setting', { key: 'first_run' })
      if (firstRun === 'true') {
        setIsFirstRun(true)
        await importWords()
        await invoke('set_setting', { key: 'first_run', value: 'false' })
      }
    } catch (e) {
      console.error('Check first run error:', e)
    }
  }

  const importWords = async () => {
    const { newbieZoneWords } = await import('./data/words')
    try {
      const count = await invoke<number>('import_word_library', { words: newbieZoneWords })
      console.log(`Imported ${count} words`)
    } catch (e) {
      console.error('Import words error:', e)
    }
  }

  const loadStats = async () => {
    try {
      const stats = await invoke('get_overall_stats')
      setOverallStats(stats)
    } catch (e) {
      console.error('Load stats error:', e)
    }
  }

  const startTraining = (type: string) => {
    setSessionType(type)
    setCurrentView('train')
  }

  const finishTraining = () => {
    setCurrentView('map')
    loadStats()
  }

  return (
    <div className="min-h-screen bg-wc-bg text-wc-text">
      {/* Header */}
      <header className="flex items-center justify-between px-6 py-3 bg-wc-surface border-b border-wc-border">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 rounded bg-gradient-to-br from-wc-primary to-wc-accent flex items-center justify-center text-sm font-bold">
            WC
          </div>
          <h1 className="text-lg font-bold tracking-wide">WordCraft</h1>
        </div>
        
        {overallStats && (
          <div className="flex items-center gap-4 text-sm">
            <div className="flex items-center gap-1.5">
              <span className="text-wc-gold">⭐</span>
              <span className="font-mono">Lv.{overallStats.level}</span>
            </div>
            <div className="flex items-center gap-1.5">
              <span className="text-wc-accent">💎</span>
              <span className="font-mono">{overallStats.total_xp} XP</span>
            </div>
            <div className="flex items-center gap-1.5">
              <span className="text-wc-fire">🔥</span>
              <span className="font-mono">{overallStats.current_streak} 天</span>
            </div>
          </div>
        )}
      </header>

      {/* Main Content */}
      <main className="p-4">
        {currentView === 'map' && (
          <AdventureMap 
            onStartTraining={startTraining}
            onOpenStats={() => setCurrentView('stats')}
            overallStats={overallStats}
          />
        )}
        
        {currentView === 'train' && (
          <WordTrainer 
            sessionType={sessionType}
            onFinish={finishTraining}
          />
        )}
        
        {currentView === 'stats' && (
          <StatsPanel onBack={() => setCurrentView('map')} />
        )}
      </main>

      {/* First Run Modal */}
      {isFirstRun && (
        <div className="fixed inset-0 bg-black/70 flex items-center justify-center z-50">
          <div className="bg-wc-surface border border-wc-border rounded-xl p-8 max-w-md w-full mx-4 pop-in">
            <h2 className="text-2xl font-bold mb-4 text-center">🎮 欢迎来到遗忘之境</h2>
            <p className="text-wc-text-muted mb-6 text-center leading-relaxed">
              你是一位掉入遗忘之境的冒险者。<br/>
              收集词汇水晶，击败遗忘魔王，<br/>
              建造属于你的家园！
            </p>
            <div className="space-y-3 mb-6">
              <div className="flex items-center gap-3 text-sm">
                <span className="text-wc-success text-lg">✨</span>
                <span>每天早中晚三个传送门自动开启</span>
              </div>
              <div className="flex items-center gap-3 text-sm">
                <span className="text-wc-accent text-lg">💎</span>
                <span>每次只需 90 秒，3-5 个单词</span>
              </div>
              <div className="flex items-center gap-3 text-sm">
                <span className="text-wc-gold text-lg">🏠</span>
                <span>收集的水晶可以用来建造家园</span>
              </div>
            </div>
            <button 
              onClick={() => setIsFirstRun(false)}
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

export default App
