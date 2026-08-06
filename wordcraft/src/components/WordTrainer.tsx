import { useState, useEffect, useRef, useCallback } from 'react'
import * as api from '../data/api'
import { gradeAnswer } from '../core/fsrs'
import { xpFor } from '../core/progression'
import { playCorrect, playIncorrect, playSessionComplete, setSoundEnabled } from '../core/sound'
import type { QueueItem, SessionType } from '../core/types'

interface WordTrainerProps {
  sessionType: SessionType
  onFinish: () => void
}

const SESSION_NAMES: Record<SessionType, string> = {
  morning: '晨曦之门',
  noon: '烈日之门',
  evening: '星夜之门',
  free: '自由探险',
}

const SESSION_COLORS: Record<SessionType, string> = {
  morning: 'from-orange-500 to-yellow-400',
  noon: 'from-yellow-500 to-amber-400',
  evening: 'from-indigo-500 to-purple-400',
  free: 'from-wc-primary to-wc-accent',
}

type Phase = 'loading' | 'error' | 'answering' | 'complete'

export default function WordTrainer({ sessionType, onFinish }: WordTrainerProps) {
  const [phase, setPhase] = useState<Phase>('loading')
  const [errorMessage, setErrorMessage] = useState('')
  const [sessionId, setSessionId] = useState<number | null>(null)

  const [queue, setQueue] = useState<QueueItem[]>([])
  const [cursor, setCursor] = useState(0)
  const [options, setOptions] = useState<string[]>([])

  const [selected, setSelected] = useState<string | null>(null)
  const [isRevealed, setIsRevealed] = useState(false)
  const [isCorrect, setIsCorrect] = useState(false)

  const [combo, setCombo] = useState(0)
  const [bestCombo, setBestCombo] = useState(0)
  const [totalXp, setTotalXp] = useState(0)
  const [answeredCount, setAnsweredCount] = useState(0)
  const [xpFloat, setXpFloat] = useState<{ xp: number; x: number; y: number } | null>(null)
  const [audioError, setAudioError] = useState('')

  const startedAt = useRef(0)
  const cardRef = useRef<HTMLDivElement>(null)

  const current = queue[cursor]

  /**
   * 组题。干扰项来自后端的同词性候选池——审计 D5 的硬编码释义数组已删除，
   * 那个数组恰好是当时 52 个词的释义，词库一扩就全线失效。
   */
  const buildOptions = useCallback(async (item: QueueItem) => {
    const pool = await api.getDistractorPool(item.word_id, item.pos, 3)
    const choices = [...pool, item.meaning]
    // Fisher-Yates 洗牌。不用 sort(() => Math.random() - 0.5)——那个比较器不满足
    // 传递性，各引擎排序实现不同，分布明显偏斜
    for (let i = choices.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1))
      ;[choices[i], choices[j]] = [choices[j], choices[i]]
    }
    setOptions(choices)
    startedAt.current = Date.now()
  }, [])

  const load = useCallback(async () => {
    setPhase('loading')
    try {
      // 静音设置在会话开始时读取一次——每题都查一次数据库没有意义，
      // 用户不会在答题中途改设置
      const sound = await api.getSetting('sound_enabled')
      setSoundEnabled(sound !== 'false')

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
      await buildOptions(items[0])
      setPhase('answering')
    } catch (e) {
      // 不回退到本地假数据（审计 D6）——后端不可用必须让用户看见
      setErrorMessage(e instanceof Error ? e.message : String(e))
      setPhase('error')
    }
  }, [sessionType, buildOptions])

  useEffect(() => {
    load()
  }, [load])

  const handleSelect = async (option: string) => {
    if (isRevealed || !current) return

    const reactionMs = Date.now() - startedAt.current
    const correct = option === current.meaning

    setSelected(option)
    setIsCorrect(correct)
    setIsRevealed(true)

    // 音效先于任何 await——spec F6 要求反馈 <100ms，
    // 排在 IPC 之后就变成「延迟到网络往返之后才响」
    if (correct) {
      playCorrect(combo)
    } else {
      playIncorrect()
    }

    const { dto, requeueInSession } = gradeAnswer({
      item: current,
      questionType: current.question_level,
      isCorrect: correct,
      reactionMs,
      sessionId,
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
      setTimeout(() => setXpFloat(null), 1000)
    }

    try {
      await api.commitReview(dto)
      // spec F2：答错的词当场排到队尾再考一次
      if (requeueInSession) {
        setQueue((q) => [...q, { ...current, app_state: dto.appState, reinforce_streak: dto.reinforceStreak }])
      }
    } catch (e) {
      setErrorMessage(e instanceof Error ? e.message : String(e))
      setPhase('error')
    }
  }

  const handleNext = async () => {
    setSelected(null)
    setIsRevealed(false)
    setIsCorrect(false)

    if (cursor < queue.length - 1) {
      const next = queue[cursor + 1]
      setCursor((c) => c + 1)
      try {
        await buildOptions(next)
      } catch (e) {
        setErrorMessage(e instanceof Error ? e.message : String(e))
        setPhase('error')
      }
      return
    }

    try {
      if (sessionId !== null) await api.finishSession(sessionId, totalXp)
      playSessionComplete()
      setPhase('complete')
    } catch (e) {
      setErrorMessage(e instanceof Error ? e.message : String(e))
      setPhase('error')
    }
  }

  const handlePlayAudio = async () => {
    if (!current) return
    try {
      setAudioError('')
      await api.playWordAudio(current.word)
    } catch (e) {
      // 发音失败不打断答题，但必须让用户看见——否则点了没反应，
      // 分不清是「坏了」还是「本来就没声音」
      setAudioError(e instanceof Error ? e.message : String(e))
    }
  }

  if (phase === 'loading') {
    return (
      <div className="flex items-center justify-center min-h-[400px]">
        <div className="text-center">
          <div className="text-4xl mb-4 animate-pulse">⚡</div>
          <p className="text-wc-text-muted">正在召唤水晶...</p>
        </div>
      </div>
    )
  }

  if (phase === 'error') {
    return (
      <div className="flex items-center justify-center min-h-[400px]">
        <div className="text-center max-w-md">
          <div className="text-4xl mb-4">🌫️</div>
          <h2 className="text-xl font-bold mb-2">传送门无法开启</h2>
          <p className="text-wc-text-muted text-sm mb-6 break-words">{errorMessage}</p>
          <div className="flex gap-3 justify-center">
            <button
              onClick={load}
              className="px-6 py-2.5 bg-gradient-to-r from-wc-primary to-wc-primary-bright rounded-lg font-bold hover:opacity-90 transition"
            >
              重试
            </button>
            <button
              onClick={onFinish}
              className="px-6 py-2.5 bg-wc-surface-2 border border-wc-border rounded-lg font-bold hover:border-wc-primary transition"
            >
              返回营地
            </button>
          </div>
        </div>
      </div>
    )
  }

  if (phase === 'complete') {
    return (
      <div className="flex items-center justify-center min-h-[500px]">
        <div className="text-center pop-in">
          <div className="text-6xl mb-4">🎉</div>
          <h2 className="text-2xl font-bold mb-2">今日冒险通关！</h2>
          <div className="bg-wc-surface border border-wc-border rounded-xl p-6 mb-6 max-w-sm mx-auto">
            <div className="grid grid-cols-2 gap-4 text-sm">
              <div>
                <div className="text-wc-text-muted">收集水晶</div>
                <div className="text-xl font-bold text-wc-accent">{answeredCount} 颗</div>
              </div>
              <div>
                <div className="text-wc-text-muted">获得 XP</div>
                <div className="text-xl font-bold text-wc-gold">{totalXp}</div>
              </div>
              <div>
                <div className="text-wc-text-muted">最高连击</div>
                <div className="text-xl font-bold text-wc-fire">{bestCombo}</div>
              </div>
              <div>
                <div className="text-wc-text-muted">传送门</div>
                <div className="text-xl font-bold">{SESSION_NAMES[sessionType]}</div>
              </div>
            </div>
          </div>
          <button
            onClick={onFinish}
            className="px-8 py-3 bg-gradient-to-r from-wc-primary to-wc-primary-bright rounded-lg font-bold hover:opacity-90 transition"
          >
            返回营地
          </button>
        </div>
      </div>
    )
  }

  if (!current) return null

  const progress = (cursor / queue.length) * 100

  return (
    <div className="max-w-lg mx-auto">
      <div className="flex items-center justify-between mb-4">
        <button onClick={onFinish} className="text-sm text-wc-text-muted hover:text-wc-text transition">
          ← 返回
        </button>
        <div
          className={`text-xs font-bold px-3 py-1 rounded-full bg-gradient-to-r ${SESSION_COLORS[sessionType]} text-white`}
        >
          {SESSION_NAMES[sessionType]}
        </div>
        <div className="text-sm font-mono">
          <span className="text-wc-accent">{cursor + 1}</span>
          <span className="text-wc-text-muted">/{queue.length}</span>
        </div>
      </div>

      <div className="h-1.5 bg-wc-surface-2 rounded-full mb-6 overflow-hidden">
        <div
          className="h-full bg-gradient-to-r from-wc-primary to-wc-accent rounded-full transition-all duration-500"
          style={{ width: `${progress}%` }}
        />
      </div>

      {combo > 0 && (
        <div className="text-center mb-4">
          <span className="inline-flex items-center gap-1 px-3 py-1 bg-wc-fire/20 text-wc-fire rounded-full text-sm font-bold">
            🔥 连击 ×{combo}
          </span>
        </div>
      )}

      <div
        ref={cardRef}
        className={`bg-wc-surface border border-wc-border rounded-xl p-6 mb-6 transition-all ${
          isRevealed && !isCorrect ? 'shake' : ''
        }`}
      >
        <div className="text-center mb-6">
          <div className="text-4xl font-bold mb-2 tracking-wide">{current.word}</div>
          <button
            className="text-wc-accent text-sm cursor-pointer hover:underline inline-flex items-center gap-1"
            onClick={handlePlayAudio}
          >
            🔊 {current.phonetic}
          </button>
          {audioError && (
            <div className="text-xs text-wc-warning mt-2 break-words">
              发音不可用：{audioError}
            </div>
          )}
        </div>

        <div
          className={`transition-all duration-500 ${
            isRevealed ? 'opacity-100 max-h-40' : 'opacity-0 max-h-0 overflow-hidden'
          }`}
        >
          <div
            className={`p-4 rounded-lg mb-4 ${
              isCorrect
                ? 'bg-wc-success/10 border border-wc-success/30'
                : 'bg-wc-danger/10 border border-wc-danger/30'
            }`}
          >
            <div className="flex items-center gap-2 mb-2">
              <span className="text-xl">{isCorrect ? '✨' : '💫'}</span>
              <span className={`font-bold ${isCorrect ? 'text-wc-success' : 'text-wc-danger'}`}>
                {isCorrect ? '水晶已点亮！' : '水晶尚未点亮...'}
              </span>
            </div>
            <div className="text-sm">
              <span className="text-wc-text-muted">释义：</span>
              <span className="font-bold">{current.meaning}</span>
              <span className="text-wc-text-muted ml-2">({current.pos})</span>
            </div>
          </div>

          <div className="text-sm text-wc-text-muted bg-wc-bg rounded-lg p-3">
            <div className="mb-1">📝 {current.example_1}</div>
            {current.example_2 && <div>📝 {current.example_2}</div>}
          </div>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-3 mb-6">
        {options.map((option, i) => {
          let btnClass = 'bg-wc-surface-2 border-wc-border hover:border-wc-primary hover:bg-wc-surface'

          if (isRevealed) {
            if (option === current.meaning) {
              btnClass = 'bg-wc-success/20 border-wc-success text-wc-success'
            } else if (option === selected) {
              btnClass = 'bg-wc-danger/20 border-wc-danger text-wc-danger'
            } else {
              btnClass = 'bg-wc-surface-2 border-wc-border opacity-50'
            }
          }

          return (
            <button
              key={`${option}-${i}`}
              onClick={() => handleSelect(option)}
              disabled={isRevealed}
              className={`p-4 rounded-lg border text-sm font-medium transition-all ${btnClass} ${
                isRevealed ? 'cursor-default' : 'cursor-pointer active:scale-95'
              }`}
            >
              {String.fromCharCode(65 + i)}. {option}
            </button>
          )
        })}
      </div>

      {isRevealed && (
        <div className="text-center pop-in">
          <button
            onClick={handleNext}
            className="px-8 py-3 bg-gradient-to-r from-wc-primary to-wc-primary-bright rounded-lg font-bold hover:opacity-90 transition"
          >
            {cursor < queue.length - 1 ? '下一个水晶 →' : '完成冒险！'}
          </button>
        </div>
      )}

      {xpFloat && (
        <div className="float-text text-wc-gold text-xl" style={{ left: xpFloat.x, top: xpFloat.y }}>
          +{xpFloat.xp} XP
        </div>
      )}
    </div>
  )
}
