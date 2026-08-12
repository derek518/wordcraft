import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, act, cleanup } from '@testing-library/react'
import CardAlbum from './CardAlbum'
import * as api from '../data/api'
import type { OverallStats } from '../core/types'

vi.mock('../core/sound', () => ({ playCorrect: vi.fn(), playLevelUp: vi.fn() }))

function entry(id: number, count: number, rarity = 1): api.CollectionEntry {
  return {
    card: {
      id,
      name: `卡${id}`,
      card_type: 'creature',
      rarity,
      image_path: `/assets/cards/common/c${id}.png`,
      trivia: '',
      source: 'CC0',
    },
    count,
    is_new: false,
    first_at: count > 0 ? '2026-08-01' : null,
  }
}

function stats(tickets: number): OverallStats {
  return {
    total_words: 3657, untouched: 3000, total_reviews: 100,
    total_xp: 100, level: 3, current_streak: 1, best_streak: 5,
    vocab_estimate: 500, draw_tickets: tickets, makeup_cards: 0,
  }
}

function stub(tickets: number, cards = [entry(1, 1), entry(2, 0)]) {
  vi.spyOn(api, 'getCollection').mockResolvedValue(cards)
  vi.spyOn(api, 'getOverallStats').mockResolvedValue(stats(tickets))
  vi.spyOn(api, 'markCardsSeen').mockResolvedValue(undefined)
}

async function settle() {
  for (let i = 0; i < 3; i++) await act(async () => { await Promise.resolve() })
}

const btn = (t: string) =>
  [...document.querySelectorAll('button')].find((b) => b.textContent?.includes(t))

beforeEach(() => vi.restoreAllMocks())
afterEach(cleanup)

describe('水晶图鉴', () => {
  it('券数为 0 时两个抽卡按钮都禁用', async () => {
    stub(0)
    render(<CardAlbum onBack={() => {}} />)
    await settle()

    expect((btn('抽一张') as HTMLButtonElement).disabled).toBe(true)
    expect((btn('十连抽') as HTMLButtonElement).disabled).toBe(true)
  })

  it('券数不足十时只禁用十连', async () => {
    stub(3)
    render(<CardAlbum onBack={() => {}} />)
    await settle()

    // 单抽够、十连不够。两个都放开会让用户点了才发现不行
    expect((btn('抽一张') as HTMLButtonElement).disabled).toBe(false)
    expect((btn('十连抽') as HTMLButtonElement).disabled).toBe(true)
  })

  it('抽完重新向后端取券数，不靠前端自减', async () => {
    const st = vi.spyOn(api, 'getOverallStats')
    stub(7)
    render(<CardAlbum onBack={() => {}} />)
    await settle()
    const before = st.mock.calls.length

    // 抽卡后后端只剩 4 张——比如另一处也消耗了券。
    // 前端若只做 7-1=6，就与真实库存错开了
    st.mockResolvedValue(stats(4))
    vi.spyOn(api, 'drawCard').mockResolvedValue({
      card: entry(1, 1).card, is_first: false, count: 2, tickets_left: 6,
    })
    await act(async () => {
      btn('抽一张')!.click()
    })
    await settle()

    expect(st.mock.calls.length).toBeGreaterThan(before)
    expect(document.body.textContent).toContain('4')
  })

  it('抽卡失败显示错误，不静默', async () => {
    stub(1)
    render(<CardAlbum onBack={() => {}} />)
    await settle()

    vi.spyOn(api, 'drawCard').mockRejectedValue(new Error('抽卡券不足'))
    await act(async () => {
      btn('抽一张')!.click()
    })
    await settle()

    expect(screen.getByText(/抽卡券不足/)).toBeTruthy()
  })

  it('后端载入失败时显示错误态，不退化成假卡池', async () => {
    vi.spyOn(api, 'getCollection').mockRejectedValue(new Error('get_collection 失败'))
    vi.spyOn(api, 'getOverallStats').mockRejectedValue(new Error('boom'))

    render(<CardAlbum onBack={() => {}} />)
    await settle()

    // 审计 D6：这里曾在 catch 里降级到 64 张本地假卡，
    // 迁移 010 崩溃那次差点就被它盖过去
    expect(screen.getByText(/get_collection 失败/)).toBeTruthy()
  })
})
