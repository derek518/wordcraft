import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, act, cleanup, fireEvent } from '@testing-library/react'
import WordLibrary from './WordLibrary'
import * as api from '../data/api'
import type { OverallStats } from '../core/types'

function stats(total: number): OverallStats {
  return {
    total_words: total, untouched: 100, total_reviews: 10,
    total_xp: 10, level: 1, current_streak: 0, best_streak: 0,
    vocab_estimate: 0, draw_tickets: 0, makeup_cards: 0,
  }
}

function stub(total = 3657) {
  vi.spyOn(api, 'getZoneProgress').mockResolvedValue([
    { key: 'newbie', name: '新手村', total: 300, learned: 0, unlocked: true },
  ] as never)
  vi.spyOn(api, 'getOverallStats').mockResolvedValue(stats(total))
  return vi.spyOn(api, 'searchWords').mockResolvedValue([])
}

async function settle() {
  for (let i = 0; i < 3; i++) await act(async () => { await Promise.resolve() })
}

const input = () => document.querySelector('input') as HTMLInputElement

beforeEach(() => {
  vi.restoreAllMocks()
  vi.useFakeTimers({ shouldAdvanceTime: true })
})
afterEach(() => {
  cleanup()
  vi.useRealTimers()
})

describe('水晶图谱', () => {
  it('词库总数取自后端，不写死', async () => {
    stub(4100)
    render(<WordLibrary onBack={() => {}} />)
    await settle()

    // 这里曾写死「3,657 个高考考纲词」。词库一更新它就成了谎话——
    // 蓝图块数、赛道积分都栽过同一件事
    expect(document.body.textContent).toContain('4,100')
  })

  it('输入停顿满 300ms 才查询，不是每敲一下都发请求', async () => {
    const search = stub()
    render(<WordLibrary onBack={() => {}} />)
    await settle()

    await act(async () => {
      fireEvent.change(input(), { target: { value: 'a' } })
    })
    await act(async () => {
      fireEvent.change(input(), { target: { value: 'ap' } })
    })
    await act(async () => {
      fireEvent.change(input(), { target: { value: 'app' } })
    })

    // **必须在窗口内推进时钟。** 不推的话，防抖设成 0ms 也不会有请求发出，
    // 测试会在防抖被去掉时照样通过
    await act(async () => {
      await vi.advanceTimersByTimeAsync(150)
    })
    expect(search).not.toHaveBeenCalled()

    await act(async () => {
      await vi.advanceTimersByTimeAsync(250)
    })

    // 只为最后的完整输入查一次，而不是三次
    expect(search).toHaveBeenCalledTimes(1)
    expect(search.mock.calls[0][0]).toBe('app')
  })

  it('清空输入不发查询，也不残留上次结果', async () => {
    const search = stub()
    render(<WordLibrary onBack={() => {}} />)
    await settle()

    await act(async () => {
      fireEvent.change(input(), { target: { value: '   ' } })
      await vi.advanceTimersByTimeAsync(350)
    })

    // 空白关键词查全库既慢又无意义
    expect(search).not.toHaveBeenCalled()
  })

  it('搜索失败显示错误，不静默变成空结果', async () => {
    stub()
    vi.spyOn(api, 'searchWords').mockRejectedValue(new Error('search_words 失败'))

    render(<WordLibrary onBack={() => {}} />)
    await settle()
    await act(async () => {
      fireEvent.change(input(), { target: { value: 'apply' } })
      await vi.advanceTimersByTimeAsync(350)
    })
    await settle()

    // 「没有匹配的词」与「查询挂了」是两回事，混为一谈会让用户找错方向
    expect(document.body.textContent).toContain('search_words 失败')
  })
})
