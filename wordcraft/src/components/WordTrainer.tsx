import { useState, useEffect, useRef, useCallback } from 'react'
import * as api from '../data/api'
import { gradeAnswer, itemAfterGrade } from '../core/fsrs'
import { xpFor } from '../core/progression'
import { playCorrect, playIncorrect, playSessionComplete, setSoundEnabled } from '../core/sound'
import { buildQuestion, checkSpelling, drillLevel, type DrillMode, type Question } from '../core/question'
import type { QueueItem, SessionType } from '../core/types'

interface WordTrainerProps {
  sessionType: SessionType
  /** 自由练习的专项模式；普通时段恒为 null */
  drillMode?: DrillMode
  onFinish: () => void
}


const SESSION_NAMES: Record<SessionType, string> = {
  morning: '晨曦之门',
  noon: '烈日之门',
  evening: '星夜之门',
  free: '自由探险',
}

/** 专项模式的标题。进入后要能一眼看出自己在练什么 */
const DRILL_NAMES: Record<Exclude<DrillMode, null>, string> = {
  spelling: '拼写专项',
  dictation: '听写模式',
}

const SESSION_COLORS: Record<SessionType, { gradient: string; glow: string }> = {
  morning: { gradient: 'from-orange-500 to-yellow-400', glow: 'rgba(251, 146, 60, 0.5)' },
  noon: { gradient: 'from-yellow-500 to-amber-400', glow: 'rgba(245, 158, 11, 0.5)' },
  evening: { gradient: 'from-indigo-500 to-purple-400', glow: 'rgba(168, 85, 247, 0.5)' },
  free: { gradient: 'from-wc-primary to-wc-accent', glow: 'rgba(124, 58, 237, 0.5)' },
}

/** 根据词频频段返回对应元素水晶图片 */
function crystalForBand(band: number, state: 'bright' | 'faint' | 'dim' = 'bright'): string {
  const elements = ['grass', 'water', 'fire', 'thunder', 'ice', 'rock']
  const idx = Math.min(Math.max(band - 1, 0), elements.length - 1)
  return `/assets/crystals/crystal_${elements[idx]}_${state}.png`
}

type Phase = 'loading' | 'error' | 'answering' | 'complete'

/** 与后端 `MAX_POSTPONE` 对齐。自由练习不走弹出调度，也不显示「稍后」。 */
const MAX_POSTPONE = 3

function optionIndexFromKey(key: string): number | null {
  if (key >= '1' && key <= '4') return Number(key) - 1
  const letter = key.toLowerCase()
  if (letter >= 'a' && letter <= 'd') return letter.charCodeAt(0) - 97
  return null
}

export default function WordTrainer({ sessionType, drillMode = null, onFinish }: WordTrainerProps) {
  // 专项模式下显示专项名——用户主动选了「听写」，标题却写「自由探险」会让人以为选错了
  const title = drillMode ? DRILL_NAMES[drillMode] : SESSION_NAMES[sessionType]
  const [phase, setPhase] = useState<Phase>('loading')
  const [errorMessage, setErrorMessage] = useState('')
  const [sessionId, setSessionId] = useState<number | null>(null)

  const [queue, setQueue] = useState<QueueItem[]>([])
  const [cursor, setCursor] = useState(0)
  const [question, setQuestion] = useState<Question | null>(null)
  const [spellInput, setSpellInput] = useState('')

  const [selected, setSelected] = useState<string | null>(null)
  const [isRevealed, setIsRevealed] = useState(false)
  const [isCorrect, setIsCorrect] = useState(false)

  const [combo, setCombo] = useState(0)
  const [bestCombo, setBestCombo] = useState(0)
  const [totalXp, setTotalXp] = useState(0)
  const [answeredCount, setAnsweredCount] = useState(0)
  const [xpFloat, setXpFloat] = useState<{ xp: number; x: number; y: number } | null>(null)
  const [audioError, setAudioError] = useState('')
  const [awaitingPrompt, setAwaitingPrompt] = useState(false)
  const [commitReady, setCommitReady] = useState(true)
  const [postponeMessage, setPostponeMessage] = useState('')

  /** 粒子爆炸状态 */
  const [particles, setParticles] = useState<{ id: number; x: number; y: number; color: string; tx: number; ty: number }[]>([])
  const particleIdRef = useRef(0)

  const audioAvailable = useRef(false)
  /** 结算是否已在进行中。用 ref 而非 state：state 更新是异步的，
   *  在它生效前重复点击仍会穿透。 */
  const finishing = useRef(false)
  const postponing = useRef(false)
  const startedAt = useRef(0)
  const cardRef = useRef<HTMLDivElement>(null)

  const current = queue[cursor]

  const spawnParticles = useCallback((x: number, y: number, color: string) => {
    const newParticles = Array.from({ length: 12 }, (_, i) => {
      const angle = (i / 12) * Math.PI * 2
      const dist = 60 + Math.random() * 60
      return {
        id: particleIdRef.current++,
        x,
        y,
        color,
        tx: Math.cos(angle) * dist,
        ty: Math.sin(angle) * dist,
      }
    })
    setParticles((prev) => [...prev, ...newParticles])
    setTimeout(() => {
      setParticles((prev) => prev.filter((p) => !newParticles.find((np) => np.id === p.id)))
    }, 900)
  }, [])

  const prepareQuestion = useCallback(async (item: QueueItem) => {
    setAwaitingPrompt(true)
    try {
      const level = drillLevel(drillMode, item, audioAvailable.current)
      const distractors = level >= 5 ? [] : await api.getDistractorPool(item.word_id, level, 3)
      setQuestion(buildQuestion({ item, level, distractors }))
      setSpellInput('')
      setAudioError('')
      // Lv.3 从音频结束起算（contracts §5）。播放失败仍开始计时，但把原因亮出来。
      if (level === 3) {
        try {
          await api.playWordAudio(item.word)
        } catch (e) {
          setAudioError(e instanceof Error ? e.message : String(e))
        }
      }
    } finally {
      startedAt.current = Date.now()
      setAwaitingPrompt(false)
    }
  }, [drillMode])

  const load = useCallback(async () => {
    setPhase('loading')
    try {
      const [sound, tts] = await Promise.all([
        api.getSetting('sound_enabled'),
        api.getSetting('tts_provider'),
      ])
      setSoundEnabled(sound !== 'false')
      audioAvailable.current = tts !== 'off'

      const items = await api.getSessionQueue(sessionType)
      if (items.length === 0) {
        setErrorMessage('词库还没有可练习的词。先在冒险者手册中导入水晶图谱。')
        setPhase('error')
        return
      }
      const session = await api.startSession(sessionType, items.length)
      setSessionId(session.id)
      setQueue(items)
      setCursor(0)
      await prepareQuestion(items[0])
      setPhase('answering')
    } catch (e) {
      setErrorMessage(e instanceof Error ? e.message : String(e))
      setPhase('error')
    }
  }, [sessionType, prepareQuestion])

  useEffect(() => {
    load()
  }, [load])

  const submitAnswer = async (input: string) => {
    if (isRevealed || awaitingPrompt || !current || !question) return

    const now = new Date()
    const reactionMs = now.getTime() - startedAt.current
    const correct =
      question.type >= 5 ? checkSpelling(input, question.answer) : input === question.answer

    setSelected(input)
    setIsCorrect(correct)
    setIsRevealed(true)

    if (correct) {
      playCorrect(combo)
      // 粒子爆炸
      if (cardRef.current) {
        const rect = cardRef.current.getBoundingClientRect()
        const centerX = rect.left + rect.width / 2
        const centerY = rect.top + rect.height / 2
        const colors = ['#22c55e', '#4ade80', '#fbbf24', '#a855f7', '#06b6d4']
        const color = colors[Math.floor(Math.random() * colors.length)]
        spawnParticles(centerX, centerY, color)
      }
    } else {
      playIncorrect()
    }

    const { dto, requeueInSession } = gradeAnswer({
      item: current,
      questionType: question.type,
      isCorrect: correct,
      reactionMs,
      sessionId,
      now,
    })

    const gained = xpFor(dto.rating, correct ? combo : 0)
    const nextCombo = correct ? combo + 1 : 0
    setCombo(nextCombo)
    setBestCombo((b) => Math.max(b, nextCombo))
    setTotalXp((x) => x + gained)
    setAnsweredCount((n) => n + 1)

    if (cardRef.current && gained > 0) {
      const rect = cardRef.current.getBoundingClientRect()
      setXpFloat({ xp: gained, x: rect.left + rect.width / 2, y: rect.top })
      setTimeout(() => setXpFloat(null), 1200)
    }

    setCommitReady(false)
    try {
      await api.commitReview(dto)
      if (requeueInSession) {
        setQueue((q) => [...q, itemAfterGrade(current, dto, now)])
      }
    } catch (e) {
      setErrorMessage(e instanceof Error ? e.message : String(e))
      setPhase('error')
    } finally {
      setCommitReady(true)
    }
  }

  const handleNext = async () => {
    if (!commitReady) return
    setSelected(null)
    setIsRevealed(false)
    setIsCorrect(false)

    if (cursor < queue.length - 1) {
      const next = queue[cursor + 1]
      setCursor((c) => c + 1)
      try {
        await prepareQuestion(next)
      } catch (e) {
        setErrorMessage(e instanceof Error ? e.message : String(e))
        setPhase('error')
      }
      return
    }

    // 防重入：handleNext 是异步的，在 finishSession 返回之前 isRevealed
    // 还没更新，按钮仍可点击——快速点两下就会发出两次结算请求。
    // 后端会拒绝重复结算（否则 XP 与抽卡券翻倍），但那个拒绝不该让用户看见
    if (finishing.current) return
    finishing.current = true

    try {
      if (sessionId !== null) await api.finishSession(sessionId, totalXp)
      playSessionComplete()
      setPhase('complete')
    } catch (e) {
      setErrorMessage(e instanceof Error ? e.message : String(e))
      setPhase('error')
      // 结算失败时放开重入，让「重试」真的能重试
      finishing.current = false
    }
  }

  const handlePostpone = async () => {
    if (sessionType === 'free' || sessionId === null || postponing.current) return
    postponing.current = true
    try {
      await api.postponeSession(sessionId)
      onFinish()
    } catch (e) {
      setPostponeMessage(e instanceof Error ? e.message : String(e))
      postponing.current = false
    }
  }

  const handlePlayAudio = async () => {
    if (!current) return
    try {
      setAudioError('')
      await api.playWordAudio(current.word)
    } catch (e) {
      setAudioError(e instanceof Error ? e.message : String(e))
    }
  }

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (phase !== 'answering' || !question || e.repeat) return
      if (isRevealed) {
        if ((e.key === ' ' || e.key === 'Enter') && commitReady) {
          e.preventDefault()
          void handleNext()
        }
        return
      }
      // 拼写框里空格是输入，不能抢走
      if (question.type >= 5 || awaitingPrompt) return
      const idx = optionIndexFromKey(e.key)
      if (idx === null) return
      const option = question.options[idx]
      if (!option) return
      e.preventDefault()
      void submitAnswer(option)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })

  const colors = SESSION_COLORS[sessionType]
  const progress = queue.length > 0 ? (cursor / queue.length) * 100 : 0

  // ============ 加载态 ============
  if (phase === 'loading') {
    return (
      <div className="flex items-center justify-center min-h-[400px]">
        <div className="text-center">
          <div className="relative w-16 h-16 mx-auto mb-6">
            <img src="/assets/crystals/crystal_fire_bright.png" alt="" className="w-full h-full object-contain animate-pulse crystal-shimmer" />
            <div className="absolute inset-0 rounded-full animate-ping opacity-20 bg-wc-primary" />
          </div>
          <p className="text-wc-text-muted text-sm tracking-wider">正在召唤水晶...</p>
        </div>
      </div>
    )
  }

  // ============ 错误态 ============
  if (phase === 'error') {
    return (
      <div className="flex items-center justify-center min-h-[400px]">
        <div className="text-center max-w-md">
          <div className="w-20 h-20 mx-auto mb-4 opacity-50">
            <img src="/assets/ui/boss.png" alt="" className="w-full h-full object-contain" />
          </div>
          <h2 className="text-xl font-bold mb-2">传送门无法开启</h2>
          <p className="text-wc-text-muted text-sm mb-6 break-words">{errorMessage}</p>
          <div className="flex gap-3 justify-center">
            <button
              onClick={load}
              className="px-6 py-2.5 bg-gradient-to-r from-wc-primary to-wc-primary-bright rounded-xl font-bold hover:opacity-90 transition btn-game"
            >
              重试
            </button>
            <button
              onClick={onFinish}
              className="px-6 py-2.5 bg-wc-surface-2 border border-wc-border rounded-xl font-bold hover:border-wc-primary transition btn-game"
            >
              返回营地
            </button>
          </div>
        </div>
      </div>
    )
  }

  // ============ 完成态 ============
  if (phase === 'complete') {
    return (
      <div className="flex items-center justify-center min-h-[500px]">
        <div className="text-center pop-in-bounce">
          <div className="relative w-24 h-24 mx-auto mb-6">
            <img src="/assets/crystals/crystal_fire_bright.png" alt="" className="w-full h-full object-contain crystal-shimmer" />
            <div className="absolute -inset-4 rounded-full animate-ping opacity-30 bg-wc-gold" />
          </div>
          <h2 className="text-3xl font-bold mb-2 tracking-wide">今日冒险通关！</h2>
          <p className="text-wc-text-muted text-sm mb-6">水晶已收集，家园等待你的归来</p>

          <div className="hud-panel rounded-2xl p-6 mb-6 max-w-sm mx-auto border border-wc-gold/20">
            <div className="grid grid-cols-2 gap-4 text-sm">
              <div className="text-center p-3 rounded-xl bg-wc-bg/50">
                <img src="/assets/crystals/crystal_water_bright.png" alt="" className="w-8 h-8 mx-auto mb-2 object-contain" />
                <div className="text-wc-text-muted text-xs">收集水晶</div>
                <div className="text-xl font-bold text-wc-accent font-game-mono">{answeredCount}</div>
              </div>
              <div className="text-center p-3 rounded-xl bg-wc-bg/50">
                <img src="/assets/effects/star.png" alt="" className="w-8 h-8 mx-auto mb-2 object-contain" />
                <div className="text-wc-text-muted text-xs">获得 XP</div>
                <div className="text-xl font-bold text-wc-gold font-game-mono">{totalXp}</div>
              </div>
              <div className="text-center p-3 rounded-xl bg-wc-bg/50">
                <div className="text-2xl mb-1">🔥</div>
                <div className="text-wc-text-muted text-xs">最高连击</div>
                <div className="text-xl font-bold text-wc-fire font-game-mono">{bestCombo}</div>
              </div>
              <div className="text-center p-3 rounded-xl bg-wc-bg/50">
                <div className="text-2xl mb-1">🚪</div>
                <div className="text-wc-text-muted text-xs">传送门</div>
                <div className="text-lg font-bold">{title}</div>
              </div>
            </div>
          </div>

          <button
            onClick={onFinish}
            className="px-10 py-3 bg-gradient-to-r from-wc-primary to-wc-primary-bright rounded-xl font-bold hover:opacity-90 transition btn-game text-lg"
            style={{ boxShadow: `0 0 20px rgba(124, 58, 237, 0.4)` }}
          >
            返回营地
          </button>
        </div>
      </div>
    )
  }

  if (!current || !question) return null

  // ============ 答题态 ============
  return (
    <div className="max-w-lg mx-auto relative">
      {/* 粒子爆炸层 */}
      {particles.map((p) => (
        <div
          key={p.id}
          className="particle"
          style={{
            left: p.x,
            top: p.y,
            backgroundColor: p.color,
            boxShadow: `0 0 6px ${p.color}`,
            '--tx': `${p.tx}px`,
            '--ty': `${p.ty}px`,
          } as React.CSSProperties}
        />
      ))}

      {/* 顶部 HUD */}
      <div className="flex items-center justify-between mb-5">
        <div className="flex items-center gap-3">
          <button
            onClick={onFinish}
            className="text-sm text-wc-text-muted hover:text-wc-text transition flex items-center gap-1"
          >
            <span>←</span> 返回
          </button>
          {sessionType !== 'free' && (
            <button
              onClick={() => void handlePostpone()}
              title={`本时段最多延后 ${MAX_POSTPONE} 次`}
              className="text-sm text-wc-text-muted hover:text-wc-text transition"
            >
              稍后
            </button>
          )}
        </div>

        <div className="flex items-center gap-2">
          {/* 传送门标识 */}
          <div
            className={`text-xs font-bold px-3 py-1 rounded-full bg-gradient-to-r ${colors.gradient} text-white`}
            style={{ boxShadow: `0 0 12px ${colors.glow}` }}
          >
            {title}
          </div>
          {/* 题型标识 */}
          <div className="text-xs px-2 py-1 rounded-lg bg-wc-surface-2 border border-wc-border text-wc-text-muted font-game-mono">
            Lv.{question.type}
          </div>
        </div>

        <div className="text-sm font-game-mono">
          <span className="text-wc-accent font-bold">{cursor + 1}</span>
          <span className="text-wc-text-muted">/{queue.length}</span>
        </div>
      </div>

      {postponeMessage && (
        <p className="text-xs text-wc-warning mb-3 break-words">{postponeMessage}</p>
      )}

      {/* 进度条 */}
      <div className="h-2 bg-wc-bg-2 rounded-full mb-5 overflow-hidden border border-wc-border/30">
        <div
          className="h-full rounded-full transition-all duration-500 relative"
          style={{
            width: `${progress}%`,
            background: `linear-gradient(90deg, #7c3aed, #a855f7, #06b6d4)`,
            boxShadow: '0 0 10px rgba(124, 58, 237, 0.5)',
          }}
        >
          <div className="absolute right-0 top-1/2 -translate-y-1/2 w-3 h-3 rounded-full bg-white shadow-[0_0_10px_white]" />
        </div>
      </div>

      {/* 连击 */}
      {combo > 0 && (
        <div className="text-center mb-4">
          <span
            className={`inline-flex items-center gap-1.5 px-4 py-1.5 rounded-full text-sm font-bold ${
              combo >= 5 ? 'combo-flame' : ''
            }`}
            style={{
              background: combo >= 5
                ? 'linear-gradient(135deg, rgba(239, 68, 68, 0.2), rgba(251, 191, 36, 0.2))'
                : 'rgba(239, 68, 68, 0.1)',
              border: `1px solid ${combo >= 5 ? 'rgba(251, 191, 36, 0.4)' : 'rgba(239, 68, 68, 0.2)'}`,
              color: combo >= 5 ? '#fbbf24' : '#f87171',
              boxShadow: combo >= 5 ? '0 0 20px rgba(251, 191, 36, 0.2)' : 'none',
            }}
          >
            <span className="text-base">🔥</span>
            连击 ×{combo}
          </span>
        </div>
      )}

      {/* 单词卡片 */}
      <div
        ref={cardRef}
        className={`relative rounded-2xl overflow-hidden mb-6 transition-all duration-300 ${
          isRevealed && !isCorrect ? 'shake-hard' : ''
        }`}
        style={{
          background: 'linear-gradient(135deg, rgba(22, 22, 42, 0.98), rgba(14, 14, 30, 0.98))',
          border: isRevealed
            ? `2px solid ${isCorrect ? 'rgba(34, 197, 94, 0.5)' : 'rgba(239, 68, 68, 0.5)'}`
            : '1px solid rgba(42, 42, 74, 0.8)',
          boxShadow: isRevealed
            ? isCorrect
              ? '0 0 30px rgba(34, 197, 94, 0.2), inset 0 0 30px rgba(34, 197, 94, 0.05)'
              : '0 0 30px rgba(239, 68, 68, 0.2), inset 0 0 30px rgba(239, 68, 68, 0.05)'
            : '0 8px 32px rgba(0, 0, 0, 0.3)',
        }}
      >
        {/* 顶部装饰线 */}
        <div
          className="h-1 w-full"
          style={{
            background: isRevealed
              ? isCorrect
                ? 'linear-gradient(90deg, transparent, #22c55e, transparent)'
                : 'linear-gradient(90deg, transparent, #ef4444, transparent)'
              : 'linear-gradient(90deg, transparent, rgba(124, 58, 237, 0.5), transparent)',
          }}
        />

        <div className="p-6">
          {/* 水晶图标 + 单词 */}
          <div className="text-center mb-5">
            {!isRevealed && (
              <div className="flex justify-center mb-3">
                <img
                  src={crystalForBand(current.frequency_band, 'bright')}
                  alt=""
                  className="w-12 h-12 object-contain crystal-shimmer"
                />
              </div>
            )}

            {question.type === 3 && !isRevealed ? (
              <button
                onClick={handlePlayAudio}
                className="text-6xl py-4 hover:scale-110 transition-transform drop-shadow-[0_0_20px_rgba(6,182,212,0.5)]"
                aria-label="播放发音"
              >
                🔊
              </button>
            ) : question.type === 4 ? (
              <div className="text-xl leading-relaxed py-2">{question.prompt}</div>
            ) : question.type >= 5 ? (
              <>
                <div className="text-2xl font-bold mb-3">{question.prompt}</div>
                <div className="text-3xl font-mono tracking-[0.3em] text-wc-text-muted">
                  {question.hint}
                </div>
              </>
            ) : question.type === 2 ? (
              <div className="text-3xl font-bold py-2">{question.prompt}</div>
            ) : (
              <>
                <div className="text-5xl font-bold mb-3 tracking-wide font-game">{current.word}</div>
                <button
                  className="text-wc-accent text-sm cursor-pointer hover:underline inline-flex items-center gap-1.5 px-3 py-1 rounded-full bg-wc-accent/10 border border-wc-accent/20 transition hover:bg-wc-accent/20"
                  onClick={handlePlayAudio}
                >
                  <span>🔊</span>
                  <span className="font-game-mono">{current.phonetic}</span>
                </button>
              </>
            )}
            {audioError && (
              <div className="text-xs text-wc-warning mt-2 break-words">
                发音不可用：{audioError}
              </div>
            )}
          </div>

          {/* 揭晓面板 */}
          <div
            className={`transition-all duration-500 ${
              isRevealed ? 'opacity-100 max-h-60' : 'opacity-0 max-h-0 overflow-hidden'
            }`}
          >
            <div
              className={`p-4 rounded-xl mb-4 border ${
                isCorrect
                  ? 'bg-wc-success/5 border-wc-success/30'
                  : 'bg-wc-danger/5 border-wc-danger/30'
              }`}
            >
              <div className="flex items-center gap-2 mb-2">
                <img
                  src={isCorrect ? '/assets/effects/star.png' : '/assets/ui/boss.png'}
                  alt=""
                  className="w-6 h-6 object-contain"
                />
                <span className={`font-bold text-lg ${isCorrect ? 'text-wc-success' : 'text-wc-danger'}`}>
                  {isCorrect ? '水晶已点亮！' : '水晶尚未点亮...'}
                </span>
              </div>
              {question.concealWord && (
                <div className="text-lg font-bold mb-1 font-game">
                  {current.word}
                  <span className="text-sm text-wc-accent font-normal ml-2 font-game-mono">
                    {current.phonetic}
                  </span>
                </div>
              )}
              <div className="text-sm">
                <span className="text-wc-text-muted">释义：</span>
                <span className="font-bold">{current.meaning}</span>
                <span className="text-wc-text-muted ml-2">({current.pos})</span>
              </div>
            </div>

            <div className="text-sm text-wc-text-muted bg-wc-bg/50 rounded-xl p-3 border border-wc-border/30">
              <div className="mb-1 flex items-start gap-2">
                <span className="text-wc-accent">📝</span>
                <span>{current.example_1}</span>
              </div>
              {current.example_2 && (
                <div className="flex items-start gap-2">
                  <span className="text-wc-accent">📝</span>
                  <span>{current.example_2}</span>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* 选项/输入区 */}
      {question.type >= 5 ? (
        <form
          className="mb-6"
          onSubmit={(e) => {
            e.preventDefault()
            if (spellInput.trim()) void submitAnswer(spellInput)
          }}
        >
          <input
            type="text"
            value={spellInput}
            onChange={(e) => setSpellInput(e.target.value)}
            disabled={isRevealed || awaitingPrompt}
            autoFocus
            autoCapitalize="off"
            autoCorrect="off"
            spellCheck={false}
            placeholder="拼出这个单词…"
            className={`w-full px-4 py-4 rounded-xl border bg-wc-surface-2 text-center text-2xl font-mono tracking-wider outline-none transition-all ${
              isRevealed
                ? isCorrect
                  ? 'border-wc-success text-wc-success'
                  : 'border-wc-danger text-wc-danger'
                : 'border-wc-border focus:border-wc-primary focus:shadow-[0_0_15px_rgba(124,58,237,0.3)]'
            }`}
          />
          {!isRevealed && (
            <button
              type="submit"
              disabled={!spellInput.trim() || awaitingPrompt}
              className="w-full mt-3 py-3 bg-gradient-to-r from-wc-primary to-wc-primary-bright rounded-xl font-bold hover:opacity-90 transition disabled:opacity-40 disabled:cursor-not-allowed btn-game"
            >
              确认
            </button>
          )}
        </form>
      ) : (
        <div className="grid grid-cols-2 gap-3 mb-6">
          {question.options.map((option, i) => {
            let btnClass = 'bg-wc-surface-2/80 border-wc-border/60 hover:border-wc-primary hover:bg-wc-surface-2'
            let btnStyle: React.CSSProperties = {}

            if (isRevealed) {
              if (option === question.answer) {
                btnClass = 'border-wc-success text-wc-success'
                btnStyle = { background: 'rgba(34, 197, 94, 0.1)', boxShadow: '0 0 15px rgba(34, 197, 94, 0.2)' }
              } else if (option === selected) {
                btnClass = 'border-wc-danger text-wc-danger'
                btnStyle = { background: 'rgba(239, 68, 68, 0.1)' }
              } else {
                btnClass = 'bg-wc-surface-2/40 border-wc-border/30 opacity-40'
              }
            }

            return (
              <button
                key={`${option}-${i}`}
                onClick={() => submitAnswer(option)}
                disabled={isRevealed || awaitingPrompt}
                className={`p-4 rounded-xl border text-sm font-medium transition-all btn-game ${btnClass} ${
                  isRevealed ? 'cursor-default' : 'cursor-pointer'
                }`}
                style={btnStyle}
              >
                <span className="inline-block w-6 h-6 rounded-full bg-wc-bg/50 text-center text-xs leading-6 mr-2 font-game-mono text-wc-text-muted">
                  {String.fromCharCode(65 + i)}
                </span>
                {option}
              </button>
            )
          })}
        </div>
      )}

      {/* 下一题按钮 */}
      {isRevealed && (
        <div className="text-center pop-in-bounce">
          <button
            onClick={() => void handleNext()}
            disabled={!commitReady}
            className="px-10 py-3 bg-gradient-to-r from-wc-primary to-wc-primary-bright rounded-xl font-bold hover:opacity-90 transition btn-game disabled:opacity-40 disabled:cursor-not-allowed"
            style={{ boxShadow: `0 0 20px rgba(124, 58, 237, 0.3)` }}
          >
            {cursor < queue.length - 1 ? '下一个水晶 →' : '完成冒险！'}
          </button>
        </div>
      )}

      {/* XP 飘字 */}
      {xpFloat && (
        <div
          className="float-xp text-wc-gold text-2xl"
          style={{ left: xpFloat.x, top: xpFloat.y }}
        >
          +{xpFloat.xp} XP
        </div>
      )}
    </div>
  )
}
