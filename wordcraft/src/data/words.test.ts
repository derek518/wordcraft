import { describe, it, expect } from 'vitest'
import { newbieZoneWords } from './words'

/**
 * 词库数据完整性。
 *
 * 这些断言与数据来源无关——T18 用真实词库（人教版 + 外研版融合）替换
 * 当前 52 词硬编码后，同一组断言依然适用，届时只需改动 import 来源。
 */
describe('词库数据完整性', () => {
  it('词库非空', () => {
    expect(newbieZoneWords.length).toBeGreaterThan(0)
  })

  it('每个词条的必需字段非空', () => {
    const required = ['word', 'phonetic', 'meaning', 'pos', 'example_1'] as const

    for (const entry of newbieZoneWords) {
      for (const field of required) {
        expect(
          entry[field],
          `词条 "${entry.word}" 的 ${field} 字段为空`,
        ).toBeTruthy()
      }
    }
  })

  it('word 字段为规范化小写英文', () => {
    for (const entry of newbieZoneWords) {
      expect(entry.word, `"${entry.word}" 不符合 /^[a-z][a-z\\-' ]*$/`).toMatch(
        /^[a-z][a-z\-' ]*$/,
      )
    }
  })

  it('音标以斜杠包裹', () => {
    for (const entry of newbieZoneWords) {
      expect(entry.phonetic, `"${entry.word}" 的音标格式异常`).toMatch(/^\/.*\/$/)
    }
  })

  it('释义不含英文字母（防止字段错位）', () => {
    for (const entry of newbieZoneWords) {
      expect(entry.meaning, `"${entry.word}" 的释义疑似串位`).not.toMatch(/[a-zA-Z]/)
    }
  })

  it('example_1 包含该词的某个词形', () => {
    for (const entry of newbieZoneWords) {
      const stem = entry.word.slice(0, Math.max(3, entry.word.length - 2))
      expect(
        entry.example_1.toLowerCase(),
        `"${entry.word}" 的例句未包含该词`,
      ).toContain(stem)
    }
  })

  it('无重复词条', () => {
    const words = newbieZoneWords.map((e) => e.word)
    expect(new Set(words).size, '存在重复单词').toBe(words.length)
  })

  it('frequency_band 在 1..5 范围内', () => {
    for (const entry of newbieZoneWords) {
      expect(entry.frequency_band).toBeGreaterThanOrEqual(1)
      expect(entry.frequency_band).toBeLessThanOrEqual(5)
    }
  })
})
