import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, act, cleanup } from '@testing-library/react'
import StatsPanel from './StatsPanel'
import * as api from '../data/api'
import type { OverallStats, DayStats, MasteryDistribution } from '../core/types'

const OVERALL: OverallStats = {
  total_words: 3657, untouched: 3000, total_reviews: 318,
  total_xp: 4554, level: 10, current_streak: 2, best_streak: 12,
  vocab_estimate: 1382, draw_tickets: 1, makeup_cards: 0,
}

const TODAY: DayStats = { total: 20, correct: 18, again: 2, hard: 3, good: 10, easy: 5 }

const MASTERY: MasteryDistribution = {
  total: 3310, untouched: 3000, learning: 96, reinforcing: 62, review: 138, mastered: 14,
}

function cells(counts: number[]) {
  const base = Date.parse('2026-08-12T00:00:00Z')
  return counts.map((count, i) => ({
    date: new Date(base - (counts.length - 1 - i) * 86400000).toISOString().slice(0, 10),
    count,
  }))
}

function stub(heat = cells([0, 3, 12, 25, 40])) {
  vi.spyOn(api, 'getTodayStats').mockResolvedValue(TODAY)
  vi.spyOn(api, 'getOverallStats').mockResolvedValue(OVERALL)
  vi.spyOn(api, 'getMasteryDistribution').mockResolvedValue(MASTERY)
  return vi.spyOn(api, 'getHeatmap').mockResolvedValue(heat)
}

async function settle() {
  for (let i = 0; i < 3; i++) await act(async () => { await Promise.resolve() })
}

/** 热力图格子：title 形如「2026-08-12　20 题」。深浅走 inline 背景色，不走 class */
const heatCells = () =>
  [...document.querySelectorAll<HTMLElement>('div[title]')].filter((el) =>
    /^\d{4}-\d{2}-\d{2}/.test(el.getAttribute('title') ?? ''),
  )
const shadesOf = () => heatCells().map((el) => el.style.backgroundColor)

beforeEach(() => vi.restoreAllMocks())
afterEach(cleanup)

describe('战绩面板', () => {
  it('热力图分档而非线性映射', async () => {
    stub(cells([0, 3, 12, 25, 40]))
    render(<StatsPanel onBack={() => {}} />)
    await settle()

    const shades = shadesOf()
    expect(shades.length).toBe(5)

    // 分五档：0 / <5 / <15 / <30 / 更多。线性映射会让绝大多数格子
    // 挤在最暗那一档——一天答 5 题和 50 题看起来一样
    expect(new Set(shades).size).toBe(5)
  })

  it('零和非零必须有肉眼可分的差别', async () => {
    stub(cells([0, 1]))
    render(<StatsPanel onBack={() => {}} />)
    await settle()

    const [zero, one] = shadesOf()
    // 练了一题和没练，是这张图最要紧的一条信息
    expect(zero).not.toBe(one)
  })

  it('后端补齐的空日期照常渲染，不塌缩', async () => {
    stub(cells([0, 0, 0, 0, 0, 0, 0]))
    render(<StatsPanel onBack={() => {}} />)
    await settle()

    // 全零周也要占满七格，否则日历会错位
    expect(heatCells().length).toBe(7)
  })

  it('载入失败显示错误，不显示一张全零的假图', async () => {
    stub()
    vi.spyOn(api, 'getOverallStats').mockRejectedValue(new Error('get_overall_stats 失败'))

    render(<StatsPanel onBack={() => {}} />)
    await settle()

    expect(document.body.textContent).toContain('get_overall_stats 失败')
  })
})
