import { describe, it, expect } from 'vitest'
import {
  blankOut,
  buildQuestion,
  checkSpelling,
  effectiveLevel,
  spellingHint,
  SPELLING_MAX_BAND,
} from './question'
import type { QueueItem, QuestionType } from './types'

function item(overrides: Partial<QueueItem> = {}): QueueItem {
  return {
    word_id: 1,
    word: 'apply',
    phonetic: '/əˈplaɪ/',
    pos: 'v.',
    meaning: '申请，应用',
    example_1: 'She applied for the guild membership last week.',
    example_2: 'Apply the paint evenly across the shield.',
    difficulty: 5,
    stability: 10,
    due_at: null,
    fsrs_state: 2,
    app_state: 'review',
    reps: 3,
    lapses: 0,
    question_level: 1,
    reinforce_streak: 0,
    frequency_band: 1,
    source: 'due_review',
    ...overrides,
  }
}

describe('实际题型等级', () => {
  it('无音频时听音辨词降为中译英', () => {
    // 没有发音还出听音题，等于让用户对着无声题面盲猜
    expect(effectiveLevel(item({ question_level: 3 }), false)).toBe(2)
    expect(effectiveLevel(item({ question_level: 3 }), true)).toBe(3)
  })

  it('拼写题只对核心词开放（决议 S10）', () => {
    for (const band of [1, 2]) {
      expect(effectiveLevel(item({ question_level: 5, frequency_band: band }), true)).toBe(5)
    }
    for (const band of [3, 4, 5]) {
      expect(effectiveLevel(item({ question_level: 5, frequency_band: band }), true)).toBe(4)
    }
    expect(SPELLING_MAX_BAND).toBe(2)
  })

  it('其余等级原样通过', () => {
    for (const lv of [1, 2, 4] as QuestionType[]) {
      expect(effectiveLevel(item({ question_level: lv }), true)).toBe(lv)
    }
  })
})

describe('例句挖空', () => {
  it('匹配词形变化而非仅原形', () => {
    // 例句里是 applied，题目考的是 apply
    expect(blankOut('She applied for the guild membership.', 'apply')).toBe(
      'She ______ for the guild membership.',
    )
  })

  it('只挖第一处，保留其余语境', () => {
    const out = blankOut('Apply the paint, then apply the varnish.', 'apply')
    expect(out).toContain('______')
    expect(out.match(/______/g)).toHaveLength(1)
    expect(out).toContain('apply the varnish')
  })

  it('词未出现时原样返回而非报错', () => {
    const sentence = 'A completely unrelated sentence.'
    expect(blankOut(sentence, 'apply')).toBe(sentence)
  })

  it('正则元字符不会破坏匹配', () => {
    expect(() => blankOut('Some text here.', 'a.b*c')).not.toThrow()
  })
})

describe('首字母提示', () => {
  it('保留首字母，其余下划线', () => {
    expect(spellingHint('apply')).toBe('a____')
    expect(spellingHint('a')).toBe('a')
  })

  it('保留空格与连字符，帮助判断词形', () => {
    expect(spellingHint('take off')).toBe('t___ ___')
    expect(spellingHint('well-known')).toBe('w___-_____')
  })
})

describe('组题', () => {
  it('一级题：显示单词，选项是释义', () => {
    const q = buildQuestion({
      item: item(),
      level: 1,
      distractors: ['放弃', '收集', '建造'],
    })
    expect(q.prompt).toBe('apply')
    expect(q.answer).toBe('申请，应用')
    expect(q.options).toHaveLength(4)
    expect(q.options).toContain('申请，应用')
    expect(q.concealWord).toBe(false)
  })

  it('二级题：显示释义，选项是单词', () => {
    const q = buildQuestion({
      item: item(),
      level: 2,
      distractors: ['abandon', 'collect', 'build'],
    })
    expect(q.prompt).toBe('申请，应用')
    expect(q.answer).toBe('apply')
    expect(q.options).toContain('apply')
    // 作答前不能显示拼写，否则退化为认读
    expect(q.concealWord).toBe(true)
  })

  it('三级题：作答前隐藏拼写', () => {
    const q = buildQuestion({ item: item(), level: 3, distractors: ['apple', 'aptly', 'ally'] })
    expect(q.concealWord).toBe(true)
    expect(q.answer).toBe('apply')
  })

  it('四级题：题干是挖空后的例句', () => {
    const q = buildQuestion({
      item: item(),
      level: 4,
      distractors: ['abandon', 'collect', 'build'],
    })
    expect(q.prompt).toContain('______')
    expect(q.prompt).not.toContain('applied')
    expect(q.answer).toBe('apply')
  })

  it('五级题：无选项，给首字母提示', () => {
    const q = buildQuestion({ item: item(), level: 5, distractors: [] })
    expect(q.options).toHaveLength(0)
    expect(q.hint).toBe('a____')
    expect(q.prompt).toBe('申请，应用')
  })

  it('干扰项不足时选项数量随之减少而非塞入空值', () => {
    const q = buildQuestion({ item: item(), level: 1, distractors: ['放弃'] })
    expect(q.options).toHaveLength(2)
    expect(q.options.every((o) => o.length > 0)).toBe(true)
  })

  it('选项顺序随机', () => {
    const positions = new Set<number>()
    for (let i = 0; i < 40; i++) {
      const q = buildQuestion({
        item: item(),
        level: 1,
        distractors: ['放弃', '收集', '建造'],
      })
      positions.add(q.options.indexOf('申请，应用'))
    }
    expect(positions.size).toBeGreaterThan(1)
  })
})

describe('拼写判定', () => {
  it('忽略大小写与首尾空白', () => {
    expect(checkSpelling('Apply', 'apply')).toBe(true)
    expect(checkSpelling('  apply  ', 'apply')).toBe(true)
  })

  it('拼错即判错，不做模糊匹配', () => {
    // 拼写题考的就是精确性，容错会让这个题型失去意义
    expect(checkSpelling('aply', 'apply')).toBe(false)
    expect(checkSpelling('applied', 'apply')).toBe(false)
    expect(checkSpelling('', 'apply')).toBe(false)
  })
})
