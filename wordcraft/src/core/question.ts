import type { QueueItem, QuestionType } from './types'

/**
 * 题型阶梯。契约见 contracts-v1.md §6。
 *
 * 题型由词的 `question_level` 决定，答对升一级、答错降一级——同一个词在不同
 * 掌握阶段面对不同难度的考查方式。
 */

/** Lv.5 全拼写的准入频段上限（决议 S10）。 */
export const SPELLING_MAX_BAND = 2

/** 需要音频支持的题型。 */
export const AUDIO_QUESTION_LEVEL = 3

export interface Question {
  type: QuestionType
  /** 题干主体：Lv.1 是单词，Lv.2 是释义，Lv.4 是挖空后的例句 */
  prompt: string
  /** 正确答案文本，也是选项之一 */
  answer: string
  /** 四个选项（含答案），已洗牌。Lv.5 为空——输入题无选项 */
  options: string[]
  /** Lv.5 的首字母提示 */
  hint?: string
  /** 是否隐藏题干中的单词拼写（Lv.3 听音辨词在作答前不显示单词） */
  concealWord: boolean
}

/**
 * 实际使用的题型等级。
 *
 * 两处降级，都不是临时妥协：
 * - **Lv.5 只对核心词开放**（决议 S10）。拼写是认知负荷最高、挫败感最强的题型，
 *   而产品目标是词汇量覆盖（认识词）而非写作产出。低频词止步 Lv.4。
 * - **Lv.3 需要音频**。发音尚未接入（MOCKS M2），此时出听音辨词题等于让用户
 *   面对无声的题面盲猜。有音频前降到 Lv.2。
 */
export function effectiveLevel(item: QueueItem, audioAvailable: boolean): QuestionType {
  let level = item.question_level

  if (level === AUDIO_QUESTION_LEVEL && !audioAvailable) {
    level = 2
  }
  if (level >= 5 && item.frequency_band > SPELLING_MAX_BAND) {
    level = 4
  }
  return level as QuestionType
}

/**
 * 自由练习的专项模式（spec §4.2 F8「拼写专项、听写模式」）。
 *
 * `null` 表示按每个词自己的等级出题，即普通训练。
 */
export type DrillMode = 'spelling' | 'dictation' | null

/** 专项模式强制的题型。 */
const DRILL_LEVEL: Record<Exclude<DrillMode, null>, QuestionType> = {
  spelling: 5,
  dictation: AUDIO_QUESTION_LEVEL,
}

/**
 * 专项模式下实际使用的题型。
 *
 * **这里刻意不套用 `effectiveLevel` 的 S10 频段限制。** 那条限制针对的是
 * 自动阶梯——系统不该擅自把低频词推到最难的题型上。专项模式是用户主动选的，
 * 选了「拼写专项」却收到选择题，只会让人以为功能坏了。
 *
 * 音频限制则保留：听写模式没有声音就是一道无解的题，不是难度问题。
 */
export function drillLevel(
  mode: DrillMode,
  item: QueueItem,
  audioAvailable: boolean,
): QuestionType {
  if (mode === null) return effectiveLevel(item, audioAvailable)
  if (mode === 'dictation' && !audioAvailable) return effectiveLevel(item, audioAvailable)
  return DRILL_LEVEL[mode]
}

/** Fisher-Yates 洗牌。 */
function shuffle<T>(items: T[]): T[] {
  const out = [...items]
  for (let i = out.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1))
    ;[out[i], out[j]] = [out[j], out[i]]
  }
  return out
}

/**
 * 在例句中挖去目标词。
 *
 * 匹配词干而非全词——例句里出现的可能是 `applied` 而题目考的是 `apply`。
 * 只挖第一处：整句的词全被挖掉会让句子失去提供语境的作用。
 */
export function blankOut(sentence: string, word: string): string {
  const stem = word.split(' ')[0].slice(0, Math.max(3, word.length - 3))
  const escaped = stem.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  return sentence.replace(new RegExp(`\\b${escaped}\\w*`, 'i'), '______')
}

/** 首字母提示：保留首字母，其余以下划线代替，保留空格与连字符。 */
export function spellingHint(word: string): string {
  return word
    .split('')
    .map((ch, i) => (i === 0 || ch === ' ' || ch === '-' ? ch : '_'))
    .join('')
}

export interface BuildQuestionInput {
  item: QueueItem
  level: QuestionType
  /** 后端 get_distractor_pool 返回的候选，语言方向已与题型匹配 */
  distractors: string[]
}

export function buildQuestion({ item, level, distractors }: BuildQuestionInput): Question {
  // Lv.1 选释义，其余选单词——与后端 distractor_pool 的返回内容保持一致
  const answer = level === 1 ? item.meaning : item.word

  if (level >= 5) {
    return {
      type: level,
      prompt: item.meaning,
      answer,
      options: [],
      hint: spellingHint(item.word),
      concealWord: true,
    }
  }

  const options = shuffle([answer, ...distractors.slice(0, 3)])

  switch (level) {
    case 2:
      return { type: level, prompt: item.meaning, answer, options, concealWord: true }

    case 3:
      // 听音辨词：作答前不能看到拼写，否则退化成认读题
      return { type: level, prompt: item.word, answer, options, concealWord: true }

    case 4:
      return {
        type: level,
        prompt: blankOut(item.example_1, item.word),
        answer,
        options,
        concealWord: true,
      }

    default:
      return { type: 1, prompt: item.word, answer, options, concealWord: false }
  }
}

/** 拼写题判定：忽略大小写与首尾空白，其余必须完全一致。 */
export function checkSpelling(input: string, word: string): boolean {
  return input.trim().toLowerCase() === word.trim().toLowerCase()
}
