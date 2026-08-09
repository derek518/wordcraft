import { useState, useEffect, useCallback, useRef } from 'react'
import * as api from '../data/api'
import { playCorrect, playIncorrect, playLevelUp } from '../core/sound'

interface BossBattleProps {
  onBack: () => void
}

/* ── 魔王主题 ── */
interface BossTheme {
  icon: string
  name: string
  color: string
  glow: string
}

function getBossTheme(lapses: number, pos: string): BossTheme {
  const tier = Math.min(Math.floor((lapses - 1) / 2), 3)
  const themes: BossTheme[] = [
    { icon: '/assets/ui/boss_tier1.png', name: '遗忘小鬼', color: '#ef4444', glow: 'rgba(239,68,68,0.4)' },
    { icon: '/assets/ui/boss_tier2.png', name: '记忆天狗', color: '#f97316', glow: 'rgba(249,115,22,0.4)' },
    { icon: '/assets/ui/boss_tier3.png', name: '遗忘巨龙', color: '#a855f7', glow: 'rgba(168,85,247,0.4)' },
    { icon: '/assets/ui/boss_tier4.png', name: '深渊魔王', color: '#dc2626', glow: 'rgba(220,38,38,0.5)' },
  ]
  const base = themes[Math.min(tier, themes.length - 1)]
  const posNames: Record<string, string> = {
    n: '名词', v: '动词', adj: '形容词', adv: '副词',
    prep: '介词', conj: '连词', pron: '代词', interj: '感叹词',
  }
  return {
    ...base,
    name: `${posNames[pos] ?? ''}${base.name}`.replace(/^$/, base.name),
  }
}

/* ── 嘲讽与胜利文案 ── */
const TAUNTS = [
  '它躲得挺好，再找一次。',
  '差一点点，它还在原地等你。',
  '这次没抓住，它跑得快。',
  '记忆有点模糊了…再想想。',
]
const VICTORY_LINES = [
  '干净利落，这个词记住了！',
  '三连击命中，收工！',
  '它再也不敢来了。',
  '完美击败！',
]

/* ── 类型 ── */
interface LogEntry {
  id: number
  text: string
  type: 'hit' | 'miss' | 'defeat' | 'info'
}

interface DamageNumber {
  id: number
  value: string
  x: number
  color: string
}

type Phase = 'loading' | 'empty' | 'error' | 'fighting' | 'victory'

let _idCounter = 0
const uid = () => ++_idCounter

export default function BossBattle({ onBack }: BossBattleProps) {
  const [phase, setPhase] = useState<Phase>('loading')
  const [bosses, setBosses] = useState<api.BossWord[]>([])
  const [current, setCurrent] = useState<api.BossWord | null>(null)
  const [options, setOptions] = useState<string[]>([])
  const [hp, setHp] = useState(0)
  const [error, setError] = useState('')
  const [taunt, setTaunt] = useState('')
  const [outcome, setOutcome] = useState<api.DefeatOutcome | null>(null)

  const [combo, setCombo] = useState(0)
  const [maxCombo, setMaxCombo] = useState(0)
  const [battleLog, setBattleLog] = useState<LogEntry[]>([])
  const [damageNumbers, setDamageNumbers] = useState<DamageNumber[]>([])
  const [bossAnim, setBossAnim] = useState<'idle' | 'hit' | 'attack'>('idle')
  const [screenShake, setScreenShake] = useState(false)
  const [transitioning, setTransitioning] = useState(false)

  const startedAt = useRef(0)
  const logRef = useRef<HTMLDivElement>(null)

  const addLog = useCallback((text: string, type: LogEntry['type']) => {
    setBattleLog((prev) => {
      const next = [...prev, { id: uid(), text, type }]
      return next.slice(-20)
    })
  }, [])

  const addDamage = useCallback((value: string, color: string) => {
    const id = uid()
    const x = (Math.random() - 0.5) * 80
    setDamageNumbers((prev) => [...prev, { id, value, x, color }])
    setTimeout(() => {
      setDamageNumbers((prev) => prev.filter((d) => d.id !== id))
    }, 950)
  }, [])

  // 三个函数按依赖顺序声明并逐层 memo 化：startFight ← engage ← load。
  // 否则 load 的依赖数组缺 engage，而 engage 每次渲染都是新引用，
  // 直接补进依赖会让 useEffect 无限重跑
  const startFight = useCallback(
    (boss: api.BossWord, pool: string[]) => {
      const choices = [...pool, boss.meaning]
      for (let i = choices.length - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1))
        ;[choices[i], choices[j]] = [choices[j], choices[i]]
      }
      setCurrent(boss)
      setOptions(choices)
      setHp(boss.hp)
      setTaunt('')
      setOutcome(null)
      setBossAnim('idle')
      setTransitioning(false)
      startedAt.current = Date.now()
      setPhase('fighting')
      addLog(`⚔️ ${getBossTheme(boss.lapses, boss.pos).name} 出现了！`, 'info')
    },
    [addLog],
  )

  const engage = useCallback(
    async (boss: api.BossWord) => {
      try {
        const pool = await api.getDistractorPool(boss.word_id, 1, 3)
        startFight(boss, pool)
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e))
        setPhase('error')
      }
    },
    [startFight],
  )

  const load = useCallback(async () => {
    setError('')
    setCombo(0)
    setMaxCombo(0)
    setBattleLog([])
    try {
      const list = await api.getBossWords(10)
      setBosses(list)
      if (list.length === 0) {
        setPhase('empty')
      } else {
        await engage(list[0])
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
      setPhase('error')
    }
  }, [engage])

  useEffect(() => {
    void load()
  }, [load])

  // 日志往末尾追加，容器有高度上限。不主动滚到底就永远停在最旧的两行，
  // 打了几拳还显示「魔王出现了」
  useEffect(() => {
    const el = logRef.current
    if (el) el.scrollTop = el.scrollHeight
  }, [battleLog])

  const strike = async (option: string) => {
    if (!current || transitioning) return
    const correct = option === current.meaning

    if (correct) {
      playCorrect(0)
      const newCombo = combo + 1
      setCombo(newCombo)
      if (newCombo > maxCombo) setMaxCombo(newCombo)

      const multiplier = Math.min(1 + newCombo * 0.1, 2.5)
      const dmg = Math.round(multiplier * 100)
      const dmgColor = newCombo >= 5 ? '#fbbf24' : newCombo >= 3 ? '#a855f7' : '#22c55e'
      addDamage(`-${dmg}`, dmgColor)

      setBossAnim('hit')
      setTimeout(() => setBossAnim('idle'), 500)

      const left = hp - 1
      setHp(left)
      addLog(`✦ 命中！连击 ×${newCombo}`, 'hit')

      if (left <= 0) {
        setTransitioning(true)
        setTimeout(async () => {
          try {
            const result = await api.defeatBoss(current.word_id)
            playLevelUp()
            setOutcome(result)
            addLog(`🎉 ${current.word} 被击败！`, 'defeat')
            setPhase('victory')
          } catch (e) {
            setError(e instanceof Error ? e.message : String(e))
            setPhase('error')
          }
        }, 600)
        return
      }

      // 立刻锁输入。500ms 的重排窗口里旧选项还在屏上，不锁就能连点同一个
      // 按钮三下打死魔王——`BOSS_HP = 3` 要的是「连对三次」，
      // 不是「同一个位置点三次」，中间必须重新认一遍
      setTransitioning(true)
      setTimeout(async () => {
        await refreshOptions(current)
        startedAt.current = Date.now()
        setTransitioning(false)
      }, 500)
    } else {
      playIncorrect()
      setBossAnim('attack')
      setScreenShake(true)
      setCombo(0)
      addDamage('MISS', '#ef4444')
      const t = TAUNTS[Math.floor(Math.random() * TAUNTS.length)]
      setTaunt(t)
      addLog(`💥 失误！魔王反击`, 'miss')

      setTransitioning(true)
      setTimeout(async () => {
        setBossAnim('idle')
        setScreenShake(false)
        setHp(current.hp)
        await refreshOptions(current)
        startedAt.current = Date.now()
        setTransitioning(false)
      }, 800)
    }
  }

  const refreshOptions = async (boss: api.BossWord) => {
    try {
      const pool = await api.getDistractorPool(boss.word_id, 1, 3)
      const choices = [...pool, boss.meaning]
      for (let i = choices.length - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1))
        ;[choices[i], choices[j]] = [choices[j], choices[i]]
      }
      setOptions(choices)
    } catch (e) {
      // 不能静默保留旧选项——那样又回到「同一个按钮点三次」。
      // 取不到新干扰项就中止本场，说明原因
      setError(e instanceof Error ? e.message : String(e))
      setPhase('error')
    }
  }

  const nextBoss = () => {
    const rest = bosses.filter((b) => b.word_id !== current?.word_id)
    setBosses(rest)
    setCombo(0)
    if (rest.length === 0) {
      setPhase('empty')
      setCurrent(null)
    } else {
      void engage(rest[0])
    }
  }

  const theme = current ? getBossTheme(current.lapses, current.pos) : null
  const optionLabels = ['A', 'B', 'C', 'D']

  /* ── Loading ── */
  if (phase === 'loading') {
    return (
      <div className="flex items-center justify-center min-h-[500px]">
        <div className="text-center">
          <div className="text-5xl mb-4 animate-pulse">⚔️</div>
          <p className="text-wc-text-muted font-game">正在搜寻魔王…</p>
          <div className="mt-4 w-48 h-1 bg-wc-surface-2 rounded-full mx-auto overflow-hidden">
            <div className="h-full progress-shine rounded-full animate-pulse" style={{ width: '60%' }} />
          </div>
        </div>
      </div>
    )
  }

  /* ── Error ── */
  if (phase === 'error') {
    return (
      <div className="flex items-center justify-center min-h-[500px]">
        <div className="text-center max-w-md pop-in-bounce">
          <div className="text-5xl mb-4">🌫️</div>
          <h2 className="text-xl font-bold mb-2 font-game">讨伐中断</h2>
          <p className="text-wc-text-muted text-sm mb-6 break-words">{error}</p>
          <button
            onClick={onBack}
            className="px-8 py-3 btn-game bg-wc-surface-2 border border-wc-border rounded-xl font-bold hover:border-wc-primary transition"
          >
            返回营地
          </button>
        </div>
      </div>
    )
  }

  /* ── Empty ── */
  if (phase === 'empty') {
    return (
      <div className="flex items-center justify-center min-h-[500px]">
        <div className="text-center max-w-sm pop-in-bounce">
          <div className="text-6xl mb-4">🕊️</div>
          <h2 className="text-2xl font-bold mb-2 font-game">境内暂无魔王</h2>
          <p className="text-wc-text-muted text-sm mb-6 leading-relaxed">
            反复忘记两次以上的词才会化为魔王。
            <br />
            现在没有，说明你记得还不错！
          </p>
          <div className="hud-panel rounded-xl p-4 mb-6 inline-block">
            <div className="text-sm text-wc-text-muted">
              本次讨伐最高连击：<span className="text-wc-gold font-game-mono font-bold">{maxCombo}</span>
            </div>
          </div>
          <br />
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

  /* ── Victory ── */
  if (phase === 'victory' && outcome && current && theme) {
    return (
      <div className="flex items-center justify-center min-h-[500px]">
        <div className="text-center pop-in-bounce max-w-sm w-full mx-4">
          <div className="relative inline-block mb-4">
            <div className="absolute inset-0 rounded-full bg-wc-gold/20 blur-2xl animate-pulse" />
            <div className="text-6xl relative z-10">🏆</div>
          </div>

          <h2 className="text-2xl font-bold mb-1 font-game">
            {VICTORY_LINES[Math.floor(Math.random() * VICTORY_LINES.length)]}
          </h2>

          <div
            className="text-4xl font-bold my-4 py-3 px-6 rounded-2xl inline-block"
            style={{
              background: `linear-gradient(135deg, ${theme.color}20, ${theme.color}08)`,
              border: `1px solid ${theme.color}40`,
              color: theme.color,
              textShadow: `0 0 20px ${theme.glow}`,
            }}
          >
            {current.word}
          </div>

          <div className="hud-panel rounded-xl p-4 mb-6 text-sm space-y-2">
            {outcome.dropped_block ? (
              <div className="flex items-center justify-center gap-2 text-wc-gold">
                <img src="/assets/blocks/block_rare.png" alt="" className="w-8 h-8 object-contain" />
                <span className="font-bold">掉落稀有方块 ×1</span>
              </div>
            ) : (
              <div className="text-wc-text-muted">这个魔王此前已讨伐过，不再掉落</div>
            )}
            <div className="text-wc-text-muted">
              题型提升至 <span className="text-wc-accent font-game-mono font-bold">Lv.{outcome.new_question_level}</span>
            </div>
            {maxCombo > 0 && (
              <div className="text-wc-gold">
                本次最高连击：<span className="font-game-mono font-bold">×{maxCombo}</span>
              </div>
            )}
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

  if (!current || !theme) return null

  /* ── Fighting ── */
  return (
    <div className={`max-w-lg mx-auto ${screenShake ? 'shake-hard' : ''}`}>
      {/* 顶部 */}
      <div className="flex items-center justify-between mb-3">
        <button
          onClick={onBack}
          className="flex items-center gap-1 text-sm text-wc-text-muted hover:text-wc-text transition"
        >
          <span className="text-lg">←</span> 撤退
        </button>
        <h2 className="text-lg font-bold font-game flex items-center gap-2">
          <span>⚔️</span>
          <span>魔王讨伐</span>
        </h2>
        <span className="text-xs text-wc-text-muted font-game-mono">剩余 {bosses.length}</span>
      </div>

      {/* 战场 */}
      <div
        className="relative rounded-2xl overflow-hidden mb-4"
        style={{
          background: `linear-gradient(180deg, ${theme.color}15 0%, rgba(10,10,20,0.95) 60%)`,
          border: `1px solid ${theme.color}30`,
          boxShadow: `0 0 30px ${theme.glow}, inset 0 0 60px rgba(0,0,0,0.5)`,
        }}
      >
        {/* 背景粒子 */}
        <div className="absolute inset-0 overflow-hidden pointer-events-none">
          {Array.from({ length: 8 }, (_, i) => (
            <div
              key={i}
              className="absolute w-1 h-1 rounded-full battle-pulse"
              style={{
                background: theme.color,
                left: `${10 + i * 12}%`,
                top: `${20 + (i % 3) * 25}%`,
                opacity: 0.3,
                animationDelay: `${i * 0.5}s`,
              }}
            />
          ))}
        </div>

        {/* 连击 */}
        {combo >= 2 && (
          <div className="absolute top-2 right-2 z-20 flex items-center gap-1 px-3 py-1.5 rounded-full bg-wc-danger/20 border border-wc-danger/40 combo-pulse">
            <span className="text-sm">{combo >= 10 ? '🔥🔥🔥' : combo >= 5 ? '🔥🔥' : '🔥'}</span>
            <span className="font-game-mono text-wc-gold font-bold text-sm">×{combo}</span>
          </div>
        )}

        {/* 魔王区 */}
        <div className="relative pt-8 pb-6 text-center">
          <div
            className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 w-32 h-32 rounded-full blur-3xl battle-pulse"
            style={{ background: theme.glow }}
          />

          <div
            className={`relative z-10 mb-3 inline-block ${
              bossAnim === 'idle' ? 'boss-float' : bossAnim === 'hit' ? 'boss-hit' : 'boss-attack'
            }`}
            style={{
              filter: `drop-shadow(0 0 20px ${theme.glow})`,
            }}
          >
            <img
              src={theme.icon}
              alt={theme.name}
              className="w-24 h-24 object-contain"
              style={{ imageRendering: 'pixelated' }}
            />
          </div>

          <div
            className="text-sm font-bold mb-1 font-game"
            style={{ color: theme.color, textShadow: `0 0 10px ${theme.glow}` }}
          >
            {theme.name}
          </div>

          <div className="text-3xl font-bold tracking-wide mb-1">{current.word}</div>
          <div className="text-sm text-wc-accent font-game-mono mb-1">{current.phonetic}</div>
          <div className="text-xs text-wc-text-muted mb-3">
            它已经击败过你 <span className="text-wc-danger font-bold">{current.lapses}</span> 次
          </div>

          {/* 血条 */}
          <div className="flex items-center gap-2 w-full max-w-[200px] mx-auto">
            <span className="text-xs text-wc-danger font-game-mono font-bold">HP</span>
            <div className="flex-1 flex gap-1">
              {Array.from({ length: current.hp }, (_, i) => (
                <div
                  key={i}
                  className={`h-3 flex-1 rounded-sm transition-all duration-300 ${
                    i < hp
                      ? 'bg-gradient-to-r from-wc-danger to-wc-danger-dim shadow-[0_0_6px_rgba(239,68,68,0.5)]'
                      : 'bg-wc-surface-3/60'
                  }`}
                />
              ))}
            </div>
            <span className="text-xs text-wc-text-muted font-game-mono">
              {hp}/{current.hp}
            </span>
          </div>

          {/* 伤害数字 */}
          {damageNumbers.map((d) => (
            <div
              key={d.id}
              className="damage-pop text-2xl"
              style={{
                color: d.color,
                left: '50%',
                top: '40%',
                transform: `translateX(${d.x}px)`,
              }}
            >
              {d.value}
            </div>
          ))}

          {/* 嘲讽 */}
          {taunt && (
            <div className="mt-3 px-4 py-2 rounded-xl bg-wc-danger/10 border border-wc-danger/20 pop-in-bounce">
              <span className="text-sm text-wc-danger">「{taunt}」</span>
            </div>
          )}
        </div>
      </div>

      {/* 选项 */}
      <div className="grid grid-cols-2 gap-3 mb-4">
        {options.map((option, i) => {
          const isCorrect = option === current.meaning
          let cls = 'bg-wc-surface-2/80 border-wc-border hover:border-wc-primary hover:bg-wc-surface-2'
          let badgeCls = 'bg-wc-surface-3 text-wc-text-muted'

          if (transitioning) {
            if (isCorrect) {
              cls = 'bg-wc-success/20 border-wc-success text-wc-success'
              badgeCls = 'bg-wc-success text-white'
            } else {
              cls = 'bg-wc-surface-2/50 border-wc-border/50 opacity-50'
            }
          }

          return (
            <button
              key={`${option}-${i}`}
              onClick={() => !transitioning && strike(option)}
              disabled={transitioning}
              className={`relative p-4 rounded-xl border text-sm font-medium transition-all ${cls} ${
                transitioning ? 'cursor-default' : 'cursor-pointer active:scale-95'
              }`}
              style={{ minHeight: '72px' }}
            >
              <div className="flex items-center gap-3">
                <span className={`option-badge ${badgeCls} transition-colors`}>{optionLabels[i]}</span>
                <span className="flex-1 text-left leading-snug">{option}</span>
              </div>
            </button>
          )
        })}
      </div>

      {/* 提示 */}
      <div className="text-center mb-3">
        <p className="text-xs text-wc-text-muted">
          连续答对 <span className="text-wc-accent font-bold">{current.hp}</span> 次即可击败
          {combo > 0 && (
            <span className="ml-2 text-wc-gold">
              · 当前连击 <span className="font-game-mono font-bold">×{combo}</span>
            </span>
          )}
        </p>
      </div>

      {/* 战斗日志 */}
      {battleLog.length > 0 && (
        <div ref={logRef} className="hud-panel rounded-xl p-3 max-h-28 overflow-y-auto">
          <div className="space-y-1">
            {battleLog.map((log) => (
              <div
                key={log.id}
                className={`text-xs font-game-mono ${
                  log.type === 'hit'
                    ? 'text-wc-success'
                    : log.type === 'miss'
                      ? 'text-wc-danger'
                      : log.type === 'defeat'
                        ? 'text-wc-gold font-bold'
                        : 'text-wc-text-muted'
                }`}
              >
                {log.text}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}
