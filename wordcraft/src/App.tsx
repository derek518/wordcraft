import { useState, useEffect, useCallback, useRef } from 'react'
import AdventureMap from './components/AdventureMap'
import WordTrainer from './components/WordTrainer'
import { fingerprintOf } from './data/libraryFingerprint'
import type { DrillMode } from './core/question'
import StatsPanel from './components/StatsPanel'
import PlacementTest from './components/PlacementTest'
import CardAlbum from './components/CardAlbum'
import SettingsPanel from './components/SettingsPanel'
import Homestead from './components/Homestead'
import SeasonTrack from './components/SeasonTrack'
import BossBattle from './components/BossBattle'
import WordLibrary from './components/WordLibrary'
import PopupPrompt from './components/PopupPrompt'
import * as api from './data/api'
import { levelProgress } from './core/progression'
import type { OverallStats, SessionType } from './core/types'
import './index.css'

type View = 'map' | 'train' | 'stats' | 'placement' | 'album' | 'settings' | 'homestead' | 'season' | 'boss' | 'library'

function isPopupMode() {
  return new URLSearchParams(window.location.search).get('mode') === 'popup'
}

export default function App() {
  if (isPopupMode()) return <PopupPrompt />
  return <MainApp />
}

function MainApp() {
  const [view, setView] = useState<View>('map')
  const [sessionType, setSessionType] = useState<SessionType>('morning')
  const [drillMode, setDrillMode] = useState<DrillMode>(null)
  const [stats, setStats] = useState<OverallStats | null>(null)
  const [showWelcome, setShowWelcome] = useState(false)
  const [bootError, setBootError] = useState('')
  const [importing, setImporting] = useState(false)
  const [importWarning, setImportWarning] = useState('')

  const loadStats = useCallback(async () => {
    try {
      setStats(await api.getOverallStats())
    } catch (e) {
      setBootError(e instanceof Error ? e.message : String(e))
    }
  }, [])

  const bootstrap = useCallback(async () => {
    try {
      const [done, storedPrint] = await Promise.all([
        api.getSetting('onboarding_done'),
        api.getSetting('library_fingerprint'),
      ])

      const res = await fetch('/library.json')
      if (!res.ok) {
        throw new Error(`词库文件读取失败（HTTP ${res.status}）`)
      }
      const raw = await res.text()
      const fingerprint = fingerprintOf(raw)

      // 词库没变就跳过。先前这里用 onboarding_done 判——一旦引导走完，
      // 词库再扩充也永远进不了老用户的库，四级词加了等于没加。
      // import_words 是按 word 的 upsert，id 保留，学习状态不受影响
      if (done === 'true' && storedPrint === fingerprint) return

      setImporting(true)
      const payload: api.WordImport[] = JSON.parse(raw)

      const outcome = await api.importWords(payload)
      if (outcome.rejected.length > 0) {
        console.warn('部分词条未通过校验：', outcome.rejected)
        const preview = outcome.rejected
          .slice(0, 3)
          .map((r) => `${r.word}（${r.reason}）`)
          .join('、')
        setImportWarning(
          `${outcome.rejected.length} 个词条未通过校验，不会出现在练习中：${preview}` +
            (outcome.rejected.length > 3 ? ' 等' : ''),
        )
      }

      await api.setSetting('library_fingerprint', fingerprint)
      if (done !== 'true') {
        await api.setSetting('onboarding_done', 'true')
        setShowWelcome(true)
      }
    } catch (e) {
      setBootError(e instanceof Error ? e.message : String(e))
    } finally {
      setImporting(false)
    }
  }, [])

  useEffect(() => {
    void (async () => {
      await bootstrap()
      await loadStats()

      try {
        const stage = await api.getSetting('placement_stage')
        if (stage !== '2') setView('placement')
      } catch (e) {
        setBootError(e instanceof Error ? e.message : String(e))
      }
    })()
  }, [bootstrap, loadStats])

  const startTraining = (type: SessionType, drill: DrillMode = null) => {
    setSessionType(type)
    setDrillMode(drill)
    setView('train')
  }

  const startRef = useRef(startTraining)
  startRef.current = startTraining

  useEffect(() => {
    let unlisten: (() => void) | undefined
    let cancelled = false
    void (async () => {
      try {
        const { listen } = await import('@tauri-apps/api/event')
        const un = await listen<SessionType>('begin-training', (e) => {
          startRef.current(e.payload)
        })
        if (cancelled) un()
        else unlisten = un
      } catch {
        // 纯浏览器 / 单测没有 Tauri 事件总线，不是后端失败
      }
    })()
    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [])

  const finishTraining = () => {
    setView('map')
    void loadStats()
  }

  const progress = stats ? levelProgress(stats.total_xp) : null

  return (
    <div className="min-h-screen bg-wc-bg text-wc-text relative">
      {/* 星空粒子背景 */}
      <div className="particle-bg" />
      {/* 扫描线覆盖 */}
      <div className="scanline-overlay" />

      {/* Header */}
      <header
        className="flex items-center justify-between px-6 py-3 relative z-10"
        style={{
          background: 'linear-gradient(180deg, rgba(22, 22, 42, 0.95), rgba(16, 16, 32, 0.98))',
          borderBottom: '1px solid rgba(42, 42, 74, 0.6)',
          backdropFilter: 'blur(10px)',
        }}
      >
        <div className="flex items-center gap-3">
          <img src="/assets/ui/app_icon_32.png" alt="" className="w-8 h-8 object-contain" />
          <h1 className="text-lg font-bold tracking-wide font-game">WordCraft</h1>
        </div>

        <div className="flex items-center gap-4 text-sm">
          {stats && progress && (
            <>
              <div className="flex items-center gap-1.5 px-3 py-1 rounded-full bg-wc-bg/50 border border-wc-border/40">
                <img src="/assets/effects/star.png" alt="" className="w-4 h-4 object-contain" />
                <span className="font-game-mono text-wc-gold">Lv.{progress.level}</span>
              </div>
              <div className="flex items-center gap-1.5 px-3 py-1 rounded-full bg-wc-bg/50 border border-wc-border/40">
                <img src="/assets/crystals/crystal_water_bright.png" alt="" className="w-4 h-4 object-contain" />
                <span className="font-game-mono text-wc-accent">{stats.total_xp}</span>
              </div>
              <div className="flex items-center gap-1.5 px-3 py-1 rounded-full bg-wc-bg/50 border border-wc-border/40">
                <span className="text-wc-fire">🔥</span>
                <span className="font-game-mono">{stats.current_streak}</span>
              </div>
            </>
          )}
          <button
            onClick={() => setView('settings')}
            className="text-wc-text-muted hover:text-wc-text transition p-1.5 rounded-lg hover:bg-wc-surface-2"
            title="设置"
          >
            <img src="/assets/blocks/block_limited.png" alt="" className="w-5 h-5 object-contain opacity-60 hover:opacity-100 transition" />
          </button>
        </div>
      </header>

      {bootError && (
        <div className="mx-4 mt-4 p-3 rounded-lg bg-wc-danger/10 border border-wc-danger/30 text-sm">
          <span className="font-bold text-wc-danger">数据加载失败：</span>
          <span className="text-wc-text-muted ml-1 break-words">{bootError}</span>
        </div>
      )}

      {importing && (
        <div className="mx-4 mt-4 p-3 rounded-lg bg-wc-surface border border-wc-border text-sm flex items-center gap-2">
          <img src="/assets/crystals/crystal_fire_bright.png" alt="" className="w-4 h-4 object-contain animate-pulse" />
          正在导入词库（3,657 词），首次启动需要几秒…
        </div>
      )}

      {importWarning && (
        <div className="mx-4 mt-4 p-3 rounded-lg bg-wc-warning/10 border border-wc-warning/30 text-sm flex items-start gap-2">
          <span>⚠️</span>
          <span className="flex-1 break-words">{importWarning}</span>
          <button
            onClick={() => setImportWarning('')}
            className="text-wc-text-muted hover:text-wc-text px-1"
          >
            ✕
          </button>
        </div>
      )}

      <main className="p-4 relative z-10">
        {view === 'map' && (
          <AdventureMap
            onStartTraining={startTraining}
            onOpenStats={() => setView('stats')}
            onOpenAlbum={() => setView('album')}
            onOpenHomestead={() => setView('homestead')}
            onOpenSeason={() => setView('season')}
            onOpenBoss={() => setView('boss')}
            onOpenLibrary={() => setView('library')}
            stats={stats}
          />
        )}
        {view === 'train' && (
          <WordTrainer sessionType={sessionType} drillMode={drillMode} onFinish={finishTraining} />
        )}
        {view === 'stats' && <StatsPanel onBack={() => setView('map')} />}
        {view === 'placement' && <PlacementTest onFinish={finishTraining} />}
        {view === 'album' && <CardAlbum onBack={() => setView('map')} />}
        {view === 'settings' && <SettingsPanel onBack={() => setView('map')} />}
        {view === 'homestead' && <Homestead onBack={() => setView('map')} />}
        {view === 'season' && <SeasonTrack onBack={() => setView('map')} />}
        {view === 'boss' && <BossBattle onBack={() => setView('map')} />}
        {view === 'library' && <WordLibrary onBack={() => setView('map')} />}
      </main>

      {/* Welcome Modal */}
      {showWelcome && (
        <div className="fixed inset-0 bg-black/80 flex items-center justify-center z-50">
          <div
            className="rounded-2xl p-8 max-w-md w-full mx-4 pop-in-bounce relative overflow-hidden"
            style={{
              background: 'linear-gradient(135deg, rgba(22, 22, 42, 0.98), rgba(14, 14, 30, 0.98))',
              border: '1px solid rgba(124, 58, 237, 0.3)',
              boxShadow: '0 0 40px rgba(124, 58, 237, 0.2), 0 20px 60px rgba(0, 0, 0, 0.5)',
            }}
          >
            {/* 装饰光晕 */}
            <div className="absolute -top-20 -right-20 w-40 h-40 rounded-full blur-3xl opacity-20 bg-wc-primary" />
            <div className="absolute -bottom-20 -left-20 w-40 h-40 rounded-full blur-3xl opacity-20 bg-wc-accent" />

            <div className="relative text-center">
              <div className="w-20 h-20 mx-auto mb-4">
                <img src="/assets/ui/app_icon_256.png" alt="" className="w-full h-full object-contain drop-shadow-[0_0_20px_rgba(168,85,247,0.5)]" />
              </div>
              <h2 className="text-2xl font-bold mb-2 tracking-wide">欢迎来到遗忘之境</h2>
              <p className="text-wc-text-muted mb-6 leading-relaxed text-sm">
                你是一位掉入遗忘之境的冒险者。<br />
                收集词汇水晶，击败遗忘魔王，<br />
                建造属于你的家园！
              </p>

              <div className="space-y-3 mb-6">
                {[
                  { icon: '/assets/crystals/crystal_grass_bright.png', text: '每天早中晚三个传送门自动开启' },
                  { icon: '/assets/crystals/crystal_fire_bright.png', text: '答对越快，水晶越亮' },
                  { icon: '/assets/blocks/block_limited.png', text: '收集的水晶可以用来建造家园' },
                ].map((item, i) => (
                  <div key={i} className="flex items-center gap-3 text-sm p-2 rounded-lg bg-wc-bg/50">
                    <img src={item.icon} alt="" className="w-6 h-6 object-contain flex-shrink-0" />
                    <span>{item.text}</span>
                  </div>
                ))}
              </div>

              <button
                onClick={() => setShowWelcome(false)}
                className="w-full py-3 bg-gradient-to-r from-wc-primary to-wc-primary-bright rounded-xl font-bold hover:opacity-90 transition btn-game"
                style={{ boxShadow: '0 0 20px rgba(124, 58, 237, 0.4)' }}
              >
                开始冒险！
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
