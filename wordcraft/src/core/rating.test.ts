import { describe, it, expect } from 'vitest'
import { autoRate, isFastCorrect, THRESHOLDS, FAST_ANSWER_MS } from './rating'
import type { QuestionType } from './types'

describe('自动评级', () => {
  it('答错一律评为 Again，与反应时间无关', () => {
    for (const qt of [1, 2, 3, 4, 5] as QuestionType[]) {
      expect(autoRate(false, 100, qt)).toBe(1)
      expect(autoRate(false, 99999, qt)).toBe(1)
    }
  })

  describe.each([
    [1 as QuestionType, 3000, 8000],
    [2 as QuestionType, 3500, 9000],
    [3 as QuestionType, 4000, 10000],
  ])('题型 Lv.%i 的阈值边界', (qt, fast, slow) => {
    it(`${fast - 1}ms 评 Easy，${fast}ms 评 Good`, () => {
      expect(autoRate(true, fast - 1, qt)).toBe(4)
      expect(autoRate(true, fast, qt)).toBe(3)
    })

    it(`${slow - 1}ms 评 Good，${slow}ms 评 Hard`, () => {
      expect(autoRate(true, slow - 1, qt)).toBe(3)
      expect(autoRate(true, slow, qt)).toBe(2)
    })
  })

  it('高阶题型答对上调一档且封顶 Easy', () => {
    // Lv.4：12000ms 本应是 Hard(2)，上调为 Good(3)
    expect(autoRate(true, 12000, 4)).toBe(3)
    // Lv.4：5000ms 本应是 Good(3)，上调为 Easy(4)
    expect(autoRate(true, 5000, 4)).toBe(4)
    // 已经是 Easy 不再上调
    expect(autoRate(true, 4999, 4)).toBe(4)
    // Lv.5 同理
    expect(autoRate(true, 20000, 5)).toBe(3)
    expect(autoRate(true, 7999, 5)).toBe(4)
  })

  it('低阶题型不享受加权', () => {
    // Lv.3 的 10000ms 是 Hard，不上调
    expect(autoRate(true, 10000, 3)).toBe(2)
  })

  it('拼写题不因打字耗时被误判（决议 S5）', () => {
    // 11 个字母的单词打字约 5 秒。用 Lv.1 阈值会被判 Good，
    // 用 Lv.5 阈值应为 Easy——完全掌握的词不该因手速缩短复习间隔
    const typingMs = 5000
    expect(autoRate(true, typingMs, 1)).toBe(3)
    expect(autoRate(true, typingMs, 5)).toBe(4)
  })

  it('阈值表覆盖全部五种题型且单调递增', () => {
    const types = [1, 2, 3, 4, 5] as QuestionType[]
    for (const qt of types) {
      expect(THRESHOLDS[qt]).toBeDefined()
      expect(THRESHOLDS[qt].fast).toBeLessThan(THRESHOLDS[qt].slow)
    }
    for (let i = 1; i < types.length; i++) {
      expect(THRESHOLDS[types[i]].fast).toBeGreaterThanOrEqual(
        THRESHOLDS[types[i - 1]].fast,
      )
    }
  })
})

describe('快速答对判定', () => {
  it('用固定 8 秒而非题型阈值', () => {
    expect(FAST_ANSWER_MS).toBe(8000)
    expect(isFastCorrect(true, 7999)).toBe(true)
    expect(isFastCorrect(true, 8000)).toBe(false)
  })

  it('答错永远不算快速答对', () => {
    expect(isFastCorrect(false, 100)).toBe(false)
  })
})
