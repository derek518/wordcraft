import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, act, cleanup } from '@testing-library/react'
import PlacementTest from './PlacementTest'
import * as api from '../data/api'

vi.mock('../core/sound', () => ({
  playCorrect: vi.fn(),
  playIncorrect: vi.fn(),
  playSessionComplete: vi.fn(),
}))

function q(id: number, rank = 2500): api.PlacementQuestion {
  return {
    word_id: id,
    word: `w${id}`,
    phonetic: '/w/',
    pos: 'n.',
    meaning: `释义${id}`,
    frequency_rank: rank,
    answered: 0,
    total: 20,
  }
}

const OUTCOME: api.AbilityOverview = {
  vocabulary: 1200, vocabulary_low: 900, vocabulary_high: 1600,
  frontier_from: 400, frontier_to: 3000,
  known: 350, frontier: 2100, too_hard: 2800,
  frontier_untouched: 1980, observations: 20,
}

async function settle() {
  for (let i = 0; i < 4; i++) await act(async () => { await Promise.resolve() })
}

const btn = (t: string) =>
  [...document.querySelectorAll('button')].find((b) => b.textContent?.includes(t))

beforeEach(() => {
  vi.restoreAllMocks()
  vi.useFakeTimers({ shouldAdvanceTime: true })
  vi.spyOn(api, 'getDistractorPool').mockResolvedValue(['甲', '乙', '丙'])
  vi.spyOn(api, 'finalizePlacement').mockResolvedValue(OUTCOME)
})
afterEach(() => {
  cleanup()
  vi.useRealTimers()
})

describe('摸底分级', () => {
  it('后端说没题了才结算，前端不自行判定结束', async () => {
    const get = vi.spyOn(api, 'getPlacementQuestion').mockResolvedValue(null)
    const fin = vi.spyOn(api, 'finalizePlacement').mockResolvedValue(OUTCOME)

    render(<PlacementTest onFinish={() => {}} />)
    await settle()

    // 摸底的推进规则（跳段、关段、连错终止）全在后端。前端若自作主张
    // 提前收尾，词汇量估算就基于不完整的样本
    expect(get).toHaveBeenCalled()
    expect(fin).toHaveBeenCalledTimes(1)
    expect(document.body.textContent).toContain('1,200')
  })

  it('答完一题继续向后端要下一题，而不是本地推进', async () => {
    const get = vi.spyOn(api, 'getPlacementQuestion')
      .mockResolvedValueOnce(q(1))
      .mockResolvedValueOnce(q(2, 2))
      .mockResolvedValue(null)
    const submit = vi.spyOn(api, 'submitPlacementAnswer')
      .mockResolvedValue({ answered: 1, total: 20, placement_done: false })

    render(<PlacementTest onFinish={() => {}} />)
    await settle()

    await act(async () => {
      btn('释义1')!.click()
    })
    await act(async () => {
      await vi.advanceTimersByTimeAsync(700)
    })
    await settle()

    expect(submit).toHaveBeenCalledWith(1, true, expect.any(Number))
    // 频段的开合由后端决定，前端只负责问「下一题是什么」
    expect(get.mock.calls.length).toBeGreaterThan(1)
  })

  it('答错也提交，且照常继续', async () => {
    vi.spyOn(api, 'getPlacementQuestion')
      .mockResolvedValueOnce(q(1))
      .mockResolvedValue(null)
    const submit = vi.spyOn(api, 'submitPlacementAnswer')
      .mockResolvedValue({ answered: 1, total: 20, placement_done: false })

    render(<PlacementTest onFinish={() => {}} />)
    await settle()

    await act(async () => {
      btn('甲')!.click() // 错误选项
    })
    await settle()

    // 答错是摸底最重要的信号——它决定频段何时关闭
    expect(submit).toHaveBeenCalledWith(1, false, expect.any(Number))
  })

  it('同一题重复点击只提交一次', async () => {
    vi.spyOn(api, 'getPlacementQuestion').mockResolvedValueOnce(q(1)).mockResolvedValue(null)
    const submit = vi.spyOn(api, 'submitPlacementAnswer')
      .mockResolvedValue({ answered: 1, total: 20, placement_done: false })

    render(<PlacementTest onFinish={() => {}} />)
    await settle()

    await act(async () => {
      btn('释义1')!.click()
      btn('甲')!.click()
    })
    await settle()

    // 重复提交会污染该频段的通过率，进而算错词汇量
    expect(submit).toHaveBeenCalledTimes(1)
  })

  it('提交失败显示错误，不带着脏状态继续问下一题', async () => {
    vi.spyOn(api, 'getPlacementQuestion').mockResolvedValue(q(1))
    vi.spyOn(api, 'submitPlacementAnswer').mockRejectedValue(new Error('写入摸底记录失败'))

    render(<PlacementTest onFinish={() => {}} />)
    await settle()

    await act(async () => {
      btn('释义1')!.click()
    })
    await settle()

    expect(document.body.textContent).toContain('写入摸底记录失败')
  })
})
