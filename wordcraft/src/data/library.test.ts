import { describe, it, expect } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

/**
 * 内置词库数据完整性。
 *
 * 断言沿用占位词库时期的那一组——它们本就与数据来源无关。数据现由
 * `scripts/wordlist/build_library.py` 从 ECDICT 考纲词汇生成（决议 S14）。
 *
 * 词库在 public/ 而非 src/：1MB 数据只在首启导入一次，走 import 会被打进
 * JS bundle。测试直接读文件，避免为了可测性把它挪回 bundle。
 */

interface Entry {
  word: string
  phonetic: string
  pos: string
  meaning: string
  example_1: string
  example_2: string
  level: string
  frequency_band: number
  zone: string
  source_edition: string
}

// 走 cwd 而非 __dirname：vitest 以项目根为工作目录，
// 而 __dirname 在 ESM 下不存在
const library: Entry[] = JSON.parse(
  readFileSync(resolve(process.cwd(), 'public/library.json'), 'utf-8'),
)

const ZONES = ['newbie', 'grass', 'water', 'fire', 'thunder', 'ice'] as const

describe('词库数据完整性', () => {
  it('规模达到考纲词汇量级', () => {
    // 实测 gk ∪ zk 去重后 3,726 词，清洗后 3,657。低于 3000 说明
    // 提取管线出了问题（决议 S2 的验证结论）
    expect(library.length).toBeGreaterThan(3000)
  })

  it('每个词条的必需字段非空', () => {
    const required = ['word', 'phonetic', 'meaning', 'pos', 'example_1'] as const
    for (const entry of library) {
      for (const field of required) {
        expect(entry[field], `词条 "${entry.word}" 的 ${field} 字段为空`).toBeTruthy()
      }
    }
  })

  it('word 字段为规范化小写英文', () => {
    for (const entry of library) {
      expect(entry.word, `"${entry.word}" 不符合 /^[a-z][a-z\\-' ]*$/`).toMatch(
        /^[a-z][a-z\-' ]*$/,
      )
    }
  })

  it('音标以斜杠包裹', () => {
    for (const entry of library) {
      expect(entry.phonetic, `"${entry.word}" 的音标格式异常`).toMatch(/^\/.*\/$/)
    }
  })

  it('释义不含英文字母（防止字段错位）', () => {
    for (const entry of library) {
      expect(entry.meaning, `"${entry.word}" 的释义疑似串位`).not.toMatch(/[a-zA-Z]/)
    }
  })

  it('example_1 包含该词的某个词形', () => {
    for (const entry of library) {
      const stem = entry.word.slice(0, Math.max(3, entry.word.length - 3))
      expect(
        entry.example_1.toLowerCase(),
        `"${entry.word}" 的例句未包含该词：${entry.example_1}`,
      ).toContain(stem.toLowerCase())
    }
  })

  it('无重复词条', () => {
    const words = library.map((e) => e.word)
    expect(new Set(words).size, '存在重复单词').toBe(words.length)
  })

  it('frequency_band 在 1..5 范围内', () => {
    for (const entry of library) {
      expect(entry.frequency_band).toBeGreaterThanOrEqual(1)
      expect(entry.frequency_band).toBeLessThanOrEqual(5)
    }
  })
})

describe('分区完整性', () => {
  it('六个区全部有词', () => {
    for (const zone of ZONES) {
      const n = library.filter((e) => e.zone === zone).length
      expect(n, `${zone} 区为空`).toBeGreaterThan(0)
    }
  })

  it('新手村恰好 50 词', () => {
    // spec §5.2 的引导设计，数量固定
    expect(library.filter((e) => e.zone === 'newbie')).toHaveLength(50)
  })

  it('无词条落在受控词表之外', () => {
    const valid = new Set([...ZONES, 'rock'])
    for (const entry of library) {
      expect(valid.has(entry.zone), `"${entry.word}" 的 zone=${entry.zone} 非法`).toBe(true)
    }
  })

  it('难度梯度成立：新手村平均词频高于高难区', () => {
    const avg = (zone: string) => {
      const items = library.filter((e) => e.zone === zone)
      return items.reduce((s, e) => s + e.frequency_band, 0) / items.length
    }
    // band 数值越小越高频，故新手村应显著低于 ice 区
    expect(avg('newbie')).toBeLessThan(avg('ice'))
    expect(avg('grass')).toBeLessThan(avg('thunder'))
  })
})

describe('例句素材约束', () => {
  it('不含商业作品的专有名词', () => {
    // spec §4：风格致敬可以，借用角色名不行。这里列举生成时最可能被
    // 模型触发的名字——它们是"游戏语境"提示下的高频联想
    const banned = [
      'Minecraft', 'Steve', 'Creeper', 'Enderman',
      'Genshin', 'Paimon', 'Zelda', 'Link', 'Mario', 'Pokemon', 'Pikachu',
      'Harry Potter', 'Hogwarts', 'Naruto', 'Sonic',
    ]
    const violations: string[] = []
    for (const entry of library) {
      const text = `${entry.example_1} ${entry.example_2}`
      for (const name of banned) {
        // 排除词条自身：`link`（链接）与 `Link`（塞尔达角色）拼写相同，
        // 不加这层判断会把正常词条误判为侵权
        if (name.toLowerCase() === entry.word.toLowerCase()) continue
        if (new RegExp(`\\b${name}\\b`, 'i').test(text)) {
          violations.push(`${entry.word}: ${name}`)
        }
      }
    }
    expect(violations, `发现受版权保护的专有名词：${violations.slice(0, 5).join('; ')}`).toHaveLength(0)
  })

  it('例句长度适中', () => {
    // 过长的句子在弹窗里显示不下，过短则缺乏语境
    for (const entry of library) {
      const words = entry.example_1.trim().split(/\s+/).length
      expect(words, `"${entry.word}" 的例句过短：${entry.example_1}`).toBeGreaterThanOrEqual(4)
      expect(words, `"${entry.word}" 的例句过长：${entry.example_1}`).toBeLessThanOrEqual(24)
    }
  })
})
