import { useState, useEffect, useRef, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'

interface WordTrainerProps {
  sessionType: string
  onFinish: () => void
}

interface WordItem {
  id: number
  word: string
  phonetic: string
  meaning: string
  pos: string
  example_1: string
  example_2: string
}

const SESSION_NAMES: Record<string, string> = {
  morning: '晨曦之门',
  noon: '烈日之门',
  evening: '星夜之门',
  free: '自由探险',
}

const SESSION_COLORS: Record<string, string> = {
  morning: 'from-orange-500 to-yellow-400',
  noon: 'from-yellow-500 to-amber-400',
  evening: 'from-indigo-500 to-purple-400',
  free: 'from-wc-primary to-wc-accent',
}

export default function WordTrainer({ sessionType, onFinish }: WordTrainerProps) {
  const [words, setWords] = useState<WordItem[]>([])
  const [currentIndex, setCurrentIndex] = useState(0)
  const [options, setOptions] = useState<string[]>([])
  const [selectedOption, setSelectedOption] = useState<string | null>(null)
  const [isRevealed, setIsRevealed] = useState(false)
  const [isCorrect, setIsCorrect] = useState(false)
  const [startTime, setStartTime] = useState(0)
  const [combo, setCombo] = useState(0)
  const [totalXp, setTotalXp] = useState(0)
  const [sessionComplete, setSessionComplete] = useState(false)
  const [showXpFloat, setShowXpFloat] = useState<{ xp: number; x: number; y: number } | null>(null)
  
  const cardRef = useRef<HTMLDivElement>(null)

  const generateOptions = useCallback((word: WordItem) => {
    // Simple distractor generation - in production this would use the database
    const allMeanings = [
      '能力，才能', '能够的', '关于', '在……上面', '接受',
      '事故', '达到', '横过', '行动', '活跃的',
      '活动', '实际上', '添加', '地址', '钦佩',
      '成年人', '前进', '优势', '冒险', '广告',
      '建议', '买得起', '害怕的', '在……之后', '下午',
      '再一次', '反对', '年龄', '以前', '同意',
      '空气', '机场', '活着的', '全部', '允许',
      '几乎', '独自', '沿着', '已经', '也',
      '虽然', '总是', '使惊奇', '在……之中', '数量',
      '古代的', '生气的', '动物', '宣布', '另一个',
    ]

    const otherMeanings = allMeanings.filter(m => m !== word.meaning)
    const shuffled = otherMeanings.sort(() => Math.random() - 0.5).slice(0, 3)
    const opts = [...shuffled, word.meaning].sort(() => Math.random() - 0.5)
    setOptions(opts)
  }, [])

  const loadWords = useCallback(async () => {
    try {
      const result = await invoke<WordItem[]>('get_due_words', {
        limit: sessionType === 'free' ? 10 : 5,
        sessionType
      })
      setWords(result)
      if (result.length > 0) {
        generateOptions(result[0])
        setStartTime(Date.now())
      }
    } catch (e) {
      console.error('Load words error:', e)
      // Fallback: use local words
      const { newbieZoneWords } = await import('../data/words')
      const localWords = newbieZoneWords.slice(0, 5).map((w: any, i: number) => ({
        id: i + 1,
        word: w.word,
        phonetic: w.phonetic,
        meaning: w.meaning,
        pos: w.pos,
        example_1: w.example_1,
        example_2: w.example_2,
      }))
      setWords(localWords)
      if (localWords.length > 0) {
        generateOptions(localWords[0])
        setStartTime(Date.now())
      }
    }
  }, [sessionType, generateOptions])

  useEffect(() => {
    loadWords()
  }, [loadWords])

  const handleSelect = async (option: string) => {
    if (isRevealed) return
    
    setSelectedOption(option)
    const correct = option === currentWord?.meaning
    setIsCorrect(correct)
    setIsRevealed(true)
    
    const reactionTime = Date.now() - startTime
    
    // Calculate rating based on reaction time and correctness
    let rating = 1 // again
    if (correct) {
      if (reactionTime < 3000) rating = 4 // easy
      else if (reactionTime < 8000) rating = 3 // good
      else rating = 2 // hard
      setCombo(c => c + 1)
    } else {
      setCombo(0)
    }
    
    // Submit to backend
    if (currentWord) {
      try {
        const result = await invoke('submit_review_result', {
          wordId: currentWord.id,
          rating,
          reactionMs: reactionTime,
        })
        const xp = (result as any)?.xp || 0
        setTotalXp(p => p + xp)
        
        // Show XP float animation
        if (cardRef.current && xp > 0) {
          const rect = cardRef.current.getBoundingClientRect()
          setShowXpFloat({ xp, x: rect.left + rect.width / 2, y: rect.top })
          setTimeout(() => setShowXpFloat(null), 1000)
        }
      } catch (e) {
        console.error('Submit review error:', e)
      }
    }
  }

  const handleNext = () => {
    if (currentIndex < words.length - 1) {
      setCurrentIndex(i => i + 1)
      setSelectedOption(null)
      setIsRevealed(false)
      setIsCorrect(false)
      setStartTime(Date.now())
      
      const nextWord = words[currentIndex + 1]
      generateOptions(nextWord)
    } else {
      setSessionComplete(true)
    }
  }

  const handlePlayAudio = async () => {
    if (!currentWord) return
    try {
      await invoke('play_word_audio', { word: currentWord.word })
    } catch (e) {
      console.error('Audio error:', e)
    }
  }

  const currentWord = words[currentIndex]
  const progress = words.length > 0 ? ((currentIndex + (isRevealed ? 1 : 0)) / words.length) * 100 : 0

  if (words.length === 0) {
    return (
      <div className="flex items-center justify-center min-h-[400px]">
        <div className="text-center">
          <div className="text-4xl mb-4 animate-pulse">⚡</div>
          <p className="text-wc-text-muted">正在召唤水晶...</p>
        </div>
      </div>
    )
  }

  if (sessionComplete) {
    return (
      <div className="flex items-center justify-center min-h-[500px]">
        <div className="text-center pop-in">
          <div className="text-6xl mb-4">🎉</div>
          <h2 className="text-2xl font-bold mb-2">今日冒险通关！</h2>
          <div className="bg-wc-surface border border-wc-border rounded-xl p-6 mb-6 max-w-sm mx-auto">
            <div className="grid grid-cols-2 gap-4 text-sm">
              <div>
                <div className="text-wc-text-muted">收集水晶</div>
                <div className="text-xl font-bold text-wc-accent">{words.length} 颗</div>
              </div>
              <div>
                <div className="text-wc-text-muted">获得 XP</div>
                <div className="text-xl font-bold text-wc-gold">{totalXp}</div>
              </div>
              <div>
                <div className="text-wc-text-muted">最高连击</div>
                <div className="text-xl font-bold text-wc-fire">{combo}</div>
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

  return (
    <div className="max-w-lg mx-auto">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <button 
          onClick={onFinish}
          className="text-sm text-wc-text-muted hover:text-wc-text transition"
        >
          ← 返回
        </button>
        <div className={`text-xs font-bold px-3 py-1 rounded-full bg-gradient-to-r ${SESSION_COLORS[sessionType]} text-white`}>
          {SESSION_NAMES[sessionType]}
        </div>
        <div className="text-sm font-mono">
          <span className="text-wc-accent">{currentIndex + 1}</span>
          <span className="text-wc-text-muted">/{words.length}</span>
        </div>
      </div>

      {/* Progress Bar */}
      <div className="h-1.5 bg-wc-surface-2 rounded-full mb-6 overflow-hidden">
        <div 
          className="h-full bg-gradient-to-r from-wc-primary to-wc-accent rounded-full transition-all duration-500"
          style={{ width: `${progress}%` }}
        />
      </div>

      {/* Combo Display */}
      {combo > 0 && (
        <div className="text-center mb-4">
          <span className="inline-flex items-center gap-1 px-3 py-1 bg-wc-fire/20 text-wc-fire rounded-full text-sm font-bold">
            🔥 连击 ×{combo}
          </span>
        </div>
      )}

      {/* Word Card */}
      <div 
        ref={cardRef}
        className={`bg-wc-surface border border-wc-border rounded-xl p-6 mb-6 transition-all ${
          isRevealed && !isCorrect ? 'shake' : ''
        }`}
      >
        <div className="text-center mb-6">
          <div className="text-4xl font-bold mb-2 tracking-wide">{currentWord.word}</div>
          <div 
            className="text-wc-accent text-sm cursor-pointer hover:underline inline-flex items-center gap-1"
            onClick={handlePlayAudio}
          >
            🔊 {currentWord.phonetic}
          </div>
        </div>

        {/* Flip Animation for Reveal */}
        <div className={`transition-all duration-500 ${isRevealed ? 'opacity-100 max-h-40' : 'opacity-0 max-h-0 overflow-hidden'}`}>
          <div className={`p-4 rounded-lg mb-4 ${isCorrect ? 'bg-wc-success/10 border border-wc-success/30' : 'bg-wc-danger/10 border border-wc-danger/30'}`}>
            <div className="flex items-center gap-2 mb-2">
              <span className="text-xl">{isCorrect ? '✨' : '💫'}</span>
              <span className={`font-bold ${isCorrect ? 'text-wc-success' : 'text-wc-danger'}`}>
                {isCorrect ? '水晶已点亮！' : '水晶尚未点亮...'}
              </span>
            </div>
            <div className="text-sm">
              <span className="text-wc-text-muted">释义：</span>
              <span className="font-bold">{currentWord.meaning}</span>
              <span className="text-wc-text-muted ml-2">({currentWord.pos})</span>
            </div>
          </div>
          
          <div className="text-sm text-wc-text-muted bg-wc-bg rounded-lg p-3">
            <div className="mb-1">📝 {currentWord.example_1}</div>
            <div>📝 {currentWord.example_2}</div>
          </div>
        </div>
      </div>

      {/* Options */}
      <div className="grid grid-cols-2 gap-3 mb-6">
        {options.map((option, i) => {
          let btnClass = 'bg-wc-surface-2 border-wc-border hover:border-wc-primary hover:bg-wc-surface'
          
          if (isRevealed) {
            if (option === currentWord.meaning) {
              btnClass = 'bg-wc-success/20 border-wc-success text-wc-success'
            } else if (option === selectedOption) {
              btnClass = 'bg-wc-danger/20 border-wc-danger text-wc-danger'
            } else {
              btnClass = 'bg-wc-surface-2 border-wc-border opacity-50'
            }
          }
          
          return (
            <button
              key={i}
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

      {/* Next Button */}
      {isRevealed && (
        <div className="text-center pop-in">
          <button
            onClick={handleNext}
            className="px-8 py-3 bg-gradient-to-r from-wc-primary to-wc-primary-bright rounded-lg font-bold hover:opacity-90 transition"
          >
            {currentIndex < words.length - 1 ? '下一个水晶 →' : '完成冒险！'}
          </button>
        </div>
      )}

      {/* XP Float Animation */}
      {showXpFloat && (
        <div 
          className="float-text text-wc-gold text-xl"
          style={{ left: showXpFloat.x, top: showXpFloat.y }}
        >
          +{showXpFloat.xp} XP
        </div>
      )}
    </div>
  )
}
