import { useState, useEffect, useCallback, useRef } from 'react'
import * as api from '../data/api'
import { playCorrect, playIncorrect, playSessionComplete } from '../core/sound'

interface PlacementTestProps {
  onFinish: () => void
}

type Phase = 'loading' | 'error' | 'asking' | 'done'

/** 「不认识」的占位标记，仅用于渲染选中态——它不会等于任何选项文本。 */
const UNKNOWN = '\u0000unknown'

/**
 * 摸底分级。contracts §9.2。
 *
 * 目的是压缩待学量而非逐词判定——60 题覆盖不了 1600 词，产出的是每层掌握率。
 * 判错的词由日常抽查纠正（§9.2④），所以这里不必追求精确。
 */
export default function PlacementTest({ onFinish }: PlacementTestProps) {
  const [phase, setPhase] = useState<Phase>('loading')
  const [errorMessage, setErrorMessage] = useState('')
  const [question, setQuestion] = useState<api.PlacementQuestion | null>(null)
  const [options, setOptions] = useState<string[]>([])
  const [selected, setSelected] = useState<string | null>(null)
  const [outcome, setOutcome] = useState<api.PlacementOutcome | null>(null)

  const startedAt = useRef(0)

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

      // 摸底固定用 Lv.1 英→中四选一（§9.2②），干扰项即取释义
      const pool = await api.getDistractorPool(q.word_id, 1, 3)
      const choices = [...pool, q.meaning]
      for (let i = choices.length - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1))
        ;[choices[i], choices[j]] = [choices[j], choices[i]]
      }

      setQuestion(q)
      setOptions(choices)
      setSelected(null)
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

  /** `option` 为 null 表示用户主动声明不认识——直接判错，不必等他乱选一个。 */
  const answer = async (option: string | null) => {
    if (!question || selected) return

    const reactionMs = Date.now() - startedAt.current
    const correct = option === question.meaning
    setSelected(option ?? UNKNOWN)

    if (correct) playCorrect(0)
    else playIncorrect()

    try {
      // 收束规则由后端判定——「连错几次结束」是产品规则，
      // 前端只需知道结果
      await api.submitPlacementAnswer(question.word_id, question.band, correct, reactionMs)
      // 留一点时间让用户看到对错反馈，再进入下一题
      setTimeout(() => void loadNext(), 450)
    } catch (e) {
      setErrorMessage(e instanceof Error ? e.message : String(e))
      setPhase('error')
    }
  }

  if (phase === 'loading') {
    return (
      <div className="flex items-center justify-center min-h-[400px]">
        <div className="text-center">
          <div className="text-4xl mb-4 animate-pulse">🔮</div>
          <p className="text-wc-text-muted">正在准备测试...</p>
        </div>
      </div>
    )
  }

  if (phase === 'error') {
    return (
      <div className="flex items-center justify-center min-h-[400px]">
        <div className="text-center max-w-md">
          <div className="text-4xl mb-4">🌫️</div>
          <h2 className="text-xl font-bold mb-2">测试无法继续</h2>
          <p className="text-wc-text-muted text-sm mb-6 break-words">{errorMessage}</p>
          <button
            onClick={onFinish}
            className="px-6 py-2.5 bg-wc-surface-2 border border-wc-border rounded-lg font-bold hover:border-wc-primary transition"
          >
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
        <div className="text-center pop-in max-w-md">
          <div className="text-6xl mb-4">🔮</div>
          <h2 className="text-2xl font-bold mb-2">水晶共鸣完成</h2>
          <p className="text-wc-text-muted text-sm mb-6">
            测试结果会随日常练习自动校正，判错的词会重新出现
          </p>

          <div className="bg-wc-surface border border-wc-border rounded-xl p-6 mb-6">
            <div className="mb-4">
              <div className="text-sm text-wc-text-muted">估算词汇量</div>
              <div className="text-4xl font-bold text-wc-accent">{outcome.vocab_estimate}</div>
            </div>
            <div className="grid grid-cols-2 gap-4 text-sm">
              <div>
                <div className="text-wc-text-muted">已点亮</div>
                <div className="text-xl font-bold text-wc-gold">{known} 词</div>
              </div>
              <div>
                <div className="text-wc-text-muted">待学习</div>
                <div className="text-xl font-bold">{outcome.skipped_new} 词</div>
              </div>
            </div>
          </div>

          <button
            onClick={onFinish}
            className="px-8 py-3 bg-gradient-to-r from-wc-primary to-wc-primary-bright rounded-lg font-bold hover:opacity-90 transition"
          >
            开始冒险！
          </button>
        </div>
      </div>
    )
  }

  if (!question) return null

  const progress = question.total > 0 ? (question.answered / question.total) * 100 : 0

  return (
    <div className="max-w-lg mx-auto">
      <div className="text-center mb-4">
        <h2 className="text-lg font-bold">🔮 水晶共鸣测试</h2>
        <p className="text-xs text-wc-text-muted mt-1">
          认识就选，不认识直接跳——测出起点才不用从头学起
        </p>
      </div>

      <div className="flex items-center justify-between text-sm mb-2">
        <span className="text-wc-text-muted">
          第 {question.answered + 1} 题
        </span>
        <span className="text-xs px-2 py-0.5 rounded bg-wc-surface-2 border border-wc-border text-wc-text-muted">
          难度 {question.band}
        </span>
      </div>

      <div className="h-1.5 bg-wc-surface-2 rounded-full mb-6 overflow-hidden">
        <div
          className="h-full bg-gradient-to-r from-wc-primary to-wc-accent rounded-full transition-all duration-500"
          style={{ width: `${progress}%` }}
        />
      </div>

      <div className="bg-wc-surface border border-wc-border rounded-xl p-8 mb-6 text-center">
        <div className="text-4xl font-bold mb-2 tracking-wide">{question.word}</div>
        <div className="text-sm text-wc-accent">{question.phonetic}</div>
      </div>

      <div className="grid grid-cols-2 gap-3 mb-4">
        {options.map((option, i) => {
          let btnClass =
            'bg-wc-surface-2 border-wc-border hover:border-wc-primary hover:bg-wc-surface'
          if (selected) {
            if (option === question.meaning) {
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
              onClick={() => answer(option)}
              disabled={selected !== null}
              className={`p-4 rounded-lg border text-sm font-medium transition-all ${btnClass} ${
                selected ? 'cursor-default' : 'cursor-pointer active:scale-95'
              }`}
            >
              {option}
            </button>
          )
        })}
      </div>

      {/* 「不认识」是必要的出口：没有它，不认识的词只能乱选一个，
          而四选一 25% 的猜中率会被记成掌握 */}
      <button
        onClick={() => answer(null)}
        disabled={selected !== null}
        className="w-full py-3 text-sm text-wc-text-muted border border-wc-border rounded-lg hover:border-wc-primary hover:text-wc-text transition disabled:opacity-40"
      >
        不认识这个词
      </button>
    </div>
  )
}
