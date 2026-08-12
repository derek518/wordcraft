import { useState, useEffect, useCallback, useRef } from 'react'
import * as api from '../data/api'
import { playCorrect, playIncorrect, playSessionComplete } from '../core/sound'

interface PlacementTestProps {
  onFinish: () => void
}

type Phase = 'loading' | 'error' | 'asking' | 'done'

const UNKNOWN = '\u0000unknown'
const BADGES = ['A', 'B', 'C', 'D']

export default function PlacementTest({ onFinish }: PlacementTestProps) {
  const [phase, setPhase] = useState<Phase>('loading')
  const [errorMessage, setErrorMessage] = useState('')
  const [question, setQuestion] = useState<api.PlacementQuestion | null>(null)
  const [options, setOptions] = useState<string[]>([])
  const [selected, setSelected] = useState<string | null>(null)
  const [outcome, setOutcome] = useState<api.PlacementOutcome | null>(null)
  const [cardGlow, setCardGlow] = useState<'default' | 'correct' | 'wrong'>('default')

  const startedAt = useRef(0)
  /** 本题是否已提交。见 answer() 里对 state 守卫为何不够的说明 */
  const submitting = useRef(false)

  const loadNext = useCallback(async () => {
    try {
      const q = await api.getPlacementQuestion()
      if (!q) {
        const result = await api.finalizePlacement()
        setOutcome(result)
        playSessionComplete()
        setPhase('done')
        return
      }

      const pool = await api.getDistractorPool(q.word_id, 1, 3)
      const choices = [...pool, q.meaning]
      for (let i = choices.length - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1))
        ;[choices[i], choices[j]] = [choices[j], choices[i]]
      }

      setQuestion(q)
      setOptions(choices)
      setSelected(null)
      setCardGlow('default')
      startedAt.current = Date.now()
      setPhase('asking')
    } catch (e) {
      setErrorMessage(e instanceof Error ? e.message : String(e))
      setPhase('error')
    }
  }, [])

  useEffect(() => {
    void loadNext()
  }, [loadNext])

  const answer = async (option: string | null) => {
    // `selected` 是 state，React 会批处理——同一 tick 内的两次点击都读到 null，
    // 于是同一题提交两次，污染该频段的通过率，进而算错词汇量。
    // ref 的赋值不经批处理，立即生效
    if (!question || selected || submitting.current) return
    submitting.current = true

    const reactionMs = Date.now() - startedAt.current
    const correct = option === question.meaning
    setSelected(option ?? UNKNOWN)
    setCardGlow(correct ? 'correct' : 'wrong')

    if (correct) playCorrect(0)
    else playIncorrect()

    try {
      await api.submitPlacementAnswer(question.word_id, question.band, correct, reactionMs)
      setTimeout(() => void loadNext(), 600)
    } catch (e) {
      setErrorMessage(e instanceof Error ? e.message : String(e))
      setPhase('error')
    } finally {
      submitting.current = false
    }
  }

  const glowColor =
    cardGlow === 'correct'
      ? 'rgba(34, 197, 94, 0.6)'
      : cardGlow === 'wrong'
        ? 'rgba(239, 68, 68, 0.6)'
        : 'rgba(124, 58, 237, 0.4)'

  if (phase === 'loading') {
    return (
      <div className="flex items-center justify-center min-h-[400px]">
        <div className="text-center">
          <img
            src="/assets/crystals/crystal_fire_bright.png"
            alt=""
            className="w-16 h-16 mx-auto mb-4 crystal-shimmer object-contain"
          />
          <p className="text-wc-text-muted font-game">正在准备测试...</p>
        </div>
      </div>
    )
  }

  if (phase === 'error') {
    return (
      <div className="flex items-center justify-center min-h-[400px]">
        <div className="text-center max-w-md hud-panel rounded-2xl p-8">
          <img
            src="/assets/crystals/crystal_rock_dim.png"
            alt=""
            className="w-16 h-16 mx-auto mb-4 opacity-50 object-contain"
          />
          <h2 className="text-xl font-bold mb-2 font-game">测试无法继续</h2>
          <p className="text-wc-text-muted text-sm mb-6 break-words">{errorMessage}</p>
          <button onClick={onFinish} className="px-6 py-2.5 btn-game bg-wc-surface-2 border border-wc-border rounded-xl font-bold hover:border-wc-primary transition">
            返回营地
          </button>
        </div>
      </div>
    )
  }

  if (phase === 'done' && outcome) {
    const known = outcome.graded_review + outcome.graded_learning
    return (
      <div className="flex items-center justify-center min-h-[500px]">
        <div className="hud-panel rounded-2xl p-8 max-w-md w-full mx-4 pop-in-bounce text-center relative overflow-hidden">
          <div className="absolute -top-20 -right-20 w-40 h-40 rounded-full blur-3xl opacity-20 bg-wc-primary" />
          <div className="absolute -bottom-20 -left-20 w-40 h-40 rounded-full blur-3xl opacity-20 bg-wc-accent" />

          <div className="relative">
            <img
              src="/assets/crystals/crystal_fire_bright.png"
              alt=""
              className="w-20 h-20 mx-auto mb-4 object-contain drop-shadow-[0_0_20px_rgba(251,191,36,0.5)]"
            />
            <h2 className="text-2xl font-bold mb-2 font-game tracking-wide">水晶共鸣完成</h2>
            <p className="text-wc-text-muted text-sm mb-6">
              测试结果会随日常练习自动校正，判错的词会重新出现
            </p>

            <div className="bg-wc-bg/50 border border-wc-border/50 rounded-xl p-6 mb-6">
              <div className="mb-4">
                <div className="text-sm text-wc-text-muted">估算词汇量</div>
                <div className="text-4xl font-bold text-wc-gold font-game-mono">{outcome.vocab_estimate}</div>
              </div>
              <div className="grid grid-cols-2 gap-4 text-sm">
                <div>
                  <div className="text-wc-text-muted">已点亮</div>
                  <div className="text-xl font-bold text-wc-success">{known} 词</div>
                </div>
                <div>
                  <div className="text-wc-text-muted">待学习</div>
                  <div className="text-xl font-bold text-wc-accent">{outcome.skipped_new} 词</div>
                </div>
              </div>
            </div>

            <button
              onClick={onFinish}
              className="w-full py-3 btn-game bg-gradient-to-r from-wc-primary to-wc-primary-bright rounded-xl font-bold"
              style={{ boxShadow: '0 0 20px rgba(124, 58, 237, 0.4)' }}
            >
              开始冒险！
            </button>
          </div>
        </div>
      </div>
    )
  }

  if (!question) return null

  const progress = question.total > 0 ? (question.answered / question.total) * 100 : 0

  return (
    <div className="max-w-lg mx-auto">
      {/* Header */}
      <div className="text-center mb-4">
        <div className="flex items-center justify-center gap-2 mb-1">
          <img src="/assets/crystals/crystal_fire_bright.png" alt="" className="w-6 h-6 object-contain" />
          <h2 className="text-lg font-bold font-game">水晶共鸣测试</h2>
        </div>
        <p className="text-xs text-wc-text-muted">
          认识就选，不认识直接跳——测出起点才不用从头学起
        </p>
      </div>

      {/* Progress */}
      <div className="flex items-center justify-between text-sm mb-2">
        <span className="text-wc-text-muted font-game-mono">第 {question.answered + 1} 题</span>
        <span className="text-xs px-2 py-0.5 rounded-lg bg-wc-surface-2 border border-wc-border text-wc-text-muted font-game-mono">
          难度 {question.band}
        </span>
      </div>

      <div className="h-2 bg-wc-surface-2 rounded-full mb-6 overflow-hidden">
        <div
          className="h-full progress-shine rounded-full transition-all duration-500"
          style={{ width: `${progress}%` }}
        />
      </div>

      {/* Word Card */}
      <div
        className="hud-panel rounded-2xl p-8 mb-6 text-center relative"
        style={{
          boxShadow: `0 0 30px ${glowColor}, inset 0 0 20px rgba(0,0,0,0.3)`,
          transition: 'box-shadow 0.4s ease',
        }}
      >
        <div
          className="absolute top-0 left-4 right-4 h-[2px] rounded-full"
          style={{
            background: `linear-gradient(90deg, transparent, ${glowColor}, transparent)`,
            transition: 'background 0.4s ease',
          }}
        />
        <div className="text-4xl font-bold mb-2 tracking-wide font-game">{question.word}</div>
        <div className="text-sm text-wc-accent font-game-mono">{question.phonetic}</div>
      </div>

      {/* Options */}
      <div className="grid grid-cols-2 gap-3 mb-4">
        {options.map((option, i) => {
          let btnClass = 'bg-wc-surface-2/80 border-wc-border hover:border-wc-primary hover:bg-wc-surface'
          let badgeColor = 'bg-wc-primary/20 text-wc-primary-bright'

          if (selected) {
            if (option === question.meaning) {
              btnClass = 'bg-wc-success/15 border-wc-success text-wc-success'
              badgeColor = 'bg-wc-success/30 text-wc-success'
            } else if (option === selected) {
              btnClass = 'bg-wc-danger/15 border-wc-danger text-wc-danger'
              badgeColor = 'bg-wc-danger/30 text-wc-danger'
            } else {
              btnClass = 'bg-wc-surface-2/40 border-wc-border/50 opacity-40'
              badgeColor = 'bg-wc-border/30 text-wc-text-muted'
            }
          }

          return (
            <button
              key={`${option}-${i}`}
              onClick={() => answer(option)}
              disabled={selected !== null}
              className={`p-4 rounded-xl border text-sm font-medium transition-all flex items-center gap-3 ${btnClass} ${
                selected ? 'cursor-default' : 'cursor-pointer btn-game active:scale-95'
              }`}
            >
              <span className={`option-badge ${badgeColor}`}>{BADGES[i]}</span>
              <span className="flex-1 text-left">{option}</span>
            </button>
          )
        })}
      </div>

      {/* Unknown */}
      <button
        onClick={() => answer(null)}
        disabled={selected !== null}
        className="w-full py-3 text-sm text-wc-text-muted border border-wc-border rounded-xl hover:border-wc-primary hover:text-wc-text transition disabled:opacity-40 btn-game"
      >
        不认识这个词
      </button>
    </div>
  )
}
