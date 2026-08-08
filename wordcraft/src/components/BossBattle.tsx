import { useState, useEffect, useCallback, useRef } from 'react'
import * as api from '../data/api'
import { playCorrect, playIncorrect, playLevelUp } from '../core/sound'

interface BossBattleProps {
  onBack: () => void
}

/**
 * 魔王嘲讽。spec 明确要求「不刻薄」。
 *
 * 目标用户已经因为记不住而受挫，真正的嘲讽只会把人推走。这些文案把矛头
 * 指向单词本身而非学习者——「这个单词很喜欢你」比「你又错了」好接受得多。
 */
const TAUNTS = [
  '这个单词看来很喜欢你呢，又来找你了。',
  '它躲得挺好，再找一次。',
  '差一点点，它还在原地等你。',
  '这次没抓住，它跑得快。',
]

const VICTORY = [
  '击败！它再也不敢来了。',
  '干净利落，这个词记住了。',
  '三连击命中，收工。',
]

type Phase = 'loading' | 'empty' | 'error' | 'fighting' | 'victory'

export default function BossBattle({ onBack }: BossBattleProps) {
  const [phase, setPhase] = useState<Phase>('loading')
  const [bosses, setBosses] = useState<api.BossWord[]>([])
  const [current, setCurrent] = useState<api.BossWord | null>(null)
  const [options, setOptions] = useState<string[]>([])
  const [selected, setSelected] = useState<string | null>(null)
  const [hp, setHp] = useState(0)
  const [error, setError] = useState('')
  const [taunt, setTaunt] = useState('')
  const [outcome, setOutcome] = useState<api.DefeatOutcome | null>(null)
  const [shake, setShake] = useState(false)

  const startedAt = useRef(0)

  const load = useCallback(async () => {
    setError('')
    try {
      const list = await api.getBossWords(10)
      setBosses(list)
      setPhase(list.length === 0 ? 'empty' : 'loading')
      if (list.length > 0) await engage(list[0])
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
      setPhase('error')
    }
  }, [])

  const engage = async (boss: api.BossWord) => {
    try {
      const pool = await api.getDistractorPool(boss.word_id, 1, 3)
      const choices = [...pool, boss.meaning]
      for (let i = choices.length - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1))
        ;[choices[i], choices[j]] = [choices[j], choices[i]]
      }
      setCurrent(boss)
      setOptions(choices)
      setHp(boss.hp)
      setSelected(null)
      setTaunt('')
      setOutcome(null)
      startedAt.current = Date.now()
      setPhase('fighting')
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
      setPhase('error')
    }
  }

  useEffect(() => {
    void load()
  }, [load])

  const strike = async (option: string) => {
    if (!current || selected) return
    const correct = option === current.meaning
    setSelected(option)

    if (correct) {
      playCorrect(0)
      const left = hp - 1
      setHp(left)

      if (left <= 0) {
        try {
          const result = await api.defeatBoss(current.word_id)
          playLevelUp()
          setOutcome(result)
          setPhase('victory')
        } catch (e) {
          setError(e instanceof Error ? e.message : String(e))
          setPhase('error')
        }
        return
      }
      // 还有血：换一批选项继续打，不换词
      setTimeout(() => {
        setSelected(null)
        startedAt.current = Date.now()
      }, 600)
    } else {
      playIncorrect()
      setShake(true)
      setTimeout(() => setShake(false), 400)
      // 答错回满血：连对三次才算真记住，中途断了要重来
      setTaunt(TAUNTS[Math.floor(Math.random() * TAUNTS.length)])
      setTimeout(() => {
        setHp(current.hp)
        setSelected(null)
        startedAt.current = Date.now()
      }, 1200)
    }
  }

  const nextBoss = () => {
    const rest = bosses.filter((b) => b.word_id !== current?.word_id)
    setBosses(rest)
    if (rest.length === 0) {
      setPhase('empty')
      setCurrent(null)
    } else {
      void engage(rest[0])
    }
  }

  if (phase === 'loading') {
    return (
      <div className="flex items-center justify-center min-h-[400px]">
        <div className="text-center">
          <div className="text-4xl mb-4 animate-pulse">⚔️</div>
          <p className="text-wc-text-muted">正在搜寻魔王…</p>
        </div>
      </div>
    )
  }

  if (phase === 'error') {
    return (
      <div className="flex items-center justify-center min-h-[400px]">
        <div className="text-center max-w-md">
          <div className="text-4xl mb-4">🌫️</div>
          <h2 className="text-xl font-bold mb-2">讨伐中断</h2>
          <p className="text-wc-text-muted text-sm mb-6 break-words">{error}</p>
          <button
            onClick={onBack}
            className="px-6 py-2.5 btn-game bg-wc-surface-2 border border-wc-border rounded-xl font-bold hover:border-wc-primary transition"
          >
            返回营地
          </button>
        </div>
      </div>
    )
  }

  if (phase === 'empty') {
    return (
      <div className="flex items-center justify-center min-h-[400px]">
        <div className="text-center max-w-sm">
          <div className="text-5xl mb-4">🕊️</div>
          <h2 className="text-xl font-bold mb-2">境内暂无魔王</h2>
          <p className="text-wc-text-muted text-sm mb-6">
            反复忘记两次以上的词才会化为魔王。
            <br />
            现在没有，说明你记得还不错。
          </p>
          <button
            onClick={onBack}
            className="px-8 py-3 btn-game bg-gradient-to-r from-wc-primary to-wc-primary-bright rounded-xl font-bold hover:opacity-90 transition"
          >
            返回营地
          </button>
        </div>
      </div>
    )
  }

  if (phase === 'victory' && outcome) {
    return (
      <div className="flex items-center justify-center min-h-[500px]">
        <div className="text-center pop-in max-w-sm">
          <div className="text-6xl mb-4">⚔️</div>
          <h2 className="text-2xl font-bold mb-1">
            {VICTORY[Math.floor(Math.random() * VICTORY.length)]}
          </h2>
          <p className="text-3xl font-bold text-wc-gold my-4">{outcome.word}</p>

          <div className="hud-panel rounded-xl p-4 mb-6 text-sm space-y-2">
            {outcome.dropped_block ? (
              <div className="flex items-center justify-center gap-2 text-wc-gold">
                <img
                  src="/assets/blocks/block_rare.png"
                  alt=""
                  className="w-8 h-8 object-contain"
                />
                掉落稀有方块 ×1
              </div>
            ) : (
              // 重复讨伐不再掉落，明说原因而不是默默不给
              <div className="text-wc-text-muted">这个魔王此前已讨伐过，不再掉落</div>
            )}
            <div className="text-wc-text-muted">
              题型提升至 Lv.{outcome.new_question_level}
            </div>
          </div>

          <div className="flex gap-3 justify-center">
            <button
              onClick={nextBoss}
              className="px-6 py-3 btn-game bg-gradient-to-r from-wc-primary to-wc-primary-bright rounded-xl font-bold hover:opacity-90 transition"
            >
              下一个魔王
            </button>
            <button
              onClick={onBack}
              className="px-6 py-3 btn-game bg-wc-surface-2 border border-wc-border rounded-xl font-bold hover:border-wc-primary transition"
            >
              收工
            </button>
          </div>
        </div>
      </div>
    )
  }

  if (!current) return null

  return (
    <div className="max-w-lg mx-auto">
      <div className="flex items-center justify-between mb-4">
        <button onClick={onBack} className="text-sm text-wc-text-muted hover:text-wc-text transition">
          ← 撤退
        </button>
        <h2 className="text-lg font-bold font-game">⚔️ 魔王讨伐</h2>
        <span className="text-xs text-wc-text-muted font-game-mono">
          剩余 {bosses.length}
        </span>
      </div>

      <div className={`hud-panel rounded-2xl p-6 mb-5 text-center ${shake ? 'shake' : ''}`}>
        {/* 血量：三格，答错回满 */}
        <div className="flex justify-center gap-2 mb-4">
          {Array.from({ length: current.hp }, (_, i) => (
            <span
              key={i}
              className={`w-8 h-2 rounded-full transition-all ${
                i < hp ? 'bg-wc-danger' : 'bg-wc-surface-3'
              }`}
            />
          ))}
        </div>

        <div className="text-5xl mb-2">👹</div>
        <div className="text-4xl font-bold tracking-wide mb-1">{current.word}</div>
        <div className="text-sm text-wc-accent mb-2">{current.phonetic}</div>
        <div className="text-xs text-wc-text-muted">
          它已经击败过你 {current.lapses} 次
        </div>

        {taunt && <div className="mt-4 text-sm text-wc-warning pop-in">「{taunt}」</div>}
      </div>

      <div className="grid grid-cols-2 gap-3">
        {options.map((option, i) => {
          let cls = 'bg-wc-surface-2 border-wc-border hover:border-wc-primary'
          if (selected) {
            if (option === current.meaning) cls = 'bg-wc-success/20 border-wc-success text-wc-success'
            else if (option === selected) cls = 'bg-wc-danger/20 border-wc-danger text-wc-danger'
            else cls = 'bg-wc-surface-2 border-wc-border opacity-50'
          }
          return (
            <button
              key={`${option}-${i}`}
              onClick={() => strike(option)}
              disabled={selected !== null}
              className={`p-4 rounded-lg border text-sm font-medium transition-all ${cls} ${
                selected ? 'cursor-default' : 'cursor-pointer active:scale-95'
              }`}
            >
              {option}
            </button>
          )
        })}
      </div>

      <p className="text-xs text-wc-text-muted text-center mt-4">
        连续答对 {current.hp} 次即可击败 · 答错血量回满
      </p>
    </div>
  )
}
