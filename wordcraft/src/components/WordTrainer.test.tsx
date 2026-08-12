import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, act, cleanup } from '@testing-library/react'
import WordTrainer from './WordTrainer'
import * as api from '../data/api'
import type { QueueItem } from '../core/types'

/**
 * 训练主循环的不变量。
 *
 * 这是整个产品的核心路径，此前没有任何测试。挑的四条各有来历——
 * 前两条对应真实发生过的故障，后两条守着「答错必须留痕」与
 * 「无音频不出无声题」这两条设计约束。
 */

vi.mock('../core/sound', () => ({
  playCorrect: vi.fn(),
  playIncorrect: vi.fn(),
  playSessionComplete: vi.fn(),
  setSoundEnabled: vi.fn(),
}))

function word(over: Partial<QueueItem> = {}): QueueItem {
  return {
    word_id: 1,
    word: 'apply',
    phonetic: '/əˈplaɪ/',
    pos: 'v.',
    meaning: '申请，应用',
    example_1: 'She applied for the guild membership.',
    example_2: '',
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
    ...over,
  }
}

const SETTINGS: Record<string, string> = {
  sound_enabled: 'false',
  tts_provider: 'edge',
}

function stub(queue: QueueItem[]) {
  vi.spyOn(api, 'getSetting').mockImplementation(async (k) => SETTINGS[k] ?? null)
  vi.spyOn(api, 'getSessionQueue').mockResolvedValue(queue)
  vi.spyOn(api, 'startSession').mockResolvedValue({ id: 7 } as never)
  vi.spyOn(api, 'getDistractorPool').mockResolvedValue(['放弃', '收集', '建造'])
  vi.spyOn(api, 'commitReview').mockResolvedValue(undefined)
  vi.spyOn(api, 'playWordAudio').mockResolvedValue(undefined)
  return vi.spyOn(api, 'finishSession').mockResolvedValue({
    completed_count: 1,
    xp_earned: 10,
    total_xp: 100,
    level: 3,
  })
}

async function settle(times = 3) {
  for (let i = 0; i < times; i++) {
    await act(async () => {
      await Promise.resolve()
    })
  }
}

beforeEach(() => vi.restoreAllMocks())
afterEach(cleanup)

const buttons = () => [...document.querySelectorAll('button')]
const byText = (t: string) => buttons().find((b) => b.textContent?.includes(t))

describe('训练主循环', () => {
  it('结算请求未返回时再次点击不会重复结算', async () => {
    const finish = stub([word()])

    // **必须让请求悬在半空。** 若 finishSession 立刻 resolve，组件已切到
    // 完成态、按钮从 DOM 卸载，第二次点击根本到不了 handleNext——
    // 守卫从未被触发，测试会在守卫被删掉时照样通过。
    // 真实场景正是「请求还没回来就又点了一下」
    let release!: () => void
    finish.mockReturnValue(
      new Promise((res) => {
        release = () => res({ completed_count: 1, xp_earned: 10, total_xp: 100, level: 3 })
      }),
    )

    render(<WordTrainer sessionType="morning" onFinish={() => {}} />)
    await settle()
    await act(async () => {
      byText('申请，应用')!.click()
    })
    await settle()

    // **两次点击必须在同一个 act 内。** handleNext 开头会
    // setIsRevealed(false)，一旦跨 act 刷新，按钮就从 DOM 卸载了，
    // 第二次点的是游离节点，守卫根本不会被触发。
    //
    // 而 finishing 是 ref，赋值不经批处理、立即生效——所以同一 tick
    // 连点正是它该防的场景。（魔王那条守的是 state，情况相反，
    // 必须跨 tick 才能复现）
    const next = buttons().find((b) => /完成|下一/.test(b.textContent ?? ''))!
    await act(async () => {
      next.click()
      next.click()
    })

    // 用户真实遇到过：发出两次结算，第二次被后端拒绝，界面弹出错误页
    expect(finish).toHaveBeenCalledTimes(1)

    await act(async () => {
      release()
      await Promise.resolve()
    })
  })

  it('结算失败后能重试成功，守卫不会把人永久锁住', async () => {
    const finish = stub([word()])
    finish.mockRejectedValueOnce(new Error('数据库忙'))

    render(<WordTrainer sessionType="morning" onFinish={() => {}} />)
    await settle()

    const finishOnce = async () => {
      await act(async () => {
        byText('申请，应用')!.click()
      })
      await settle()
      await act(async () => {
        buttons().find((b) => /完成|下一/.test(b.textContent ?? ''))!.click()
      })
      await settle()
    }

    await finishOnce()
    expect(screen.getByText(/数据库忙/)).toBeTruthy()

    // 重试重开一场。`load` 不会重置 finishing，所以守卫若不在失败时放开，
    // 这一场将永远结算不掉——错误态里的「重试」成了摆设
    await act(async () => {
      buttons().find((b) => /重试|再试/.test(b.textContent ?? ''))!.click()
    })
    await settle()
    await finishOnce()

    expect(finish).toHaveBeenCalledTimes(2)
  })

  it('答错也提交作答记录', async () => {
    stub([word()])
    const commit = vi.spyOn(api, 'commitReview').mockResolvedValue(undefined)
    render(<WordTrainer sessionType="morning" onFinish={() => {}} />)
    await settle()

    // 选项带字母前缀，形如「A收集」
    const wrong = buttons().find((b) =>
      /^[A-D](放弃|收集|建造)$/.test((b.textContent ?? '').trim()),
    )
    expect(wrong).toBeDefined()
    await act(async () => {
      wrong!.click()
    })
    await settle()

    // 答错是 FSRS 最重要的输入之一。只提交答对的记录，
    // 遗忘曲线会整体偏乐观，复习间隔越拉越长
    expect(commit).toHaveBeenCalledTimes(1)
    expect(commit.mock.calls[0][0].isCorrect).toBe(false)
  })

  it('关闭 TTS 时不出听音辨词题', async () => {
    SETTINGS.tts_provider = 'off'
    stub([word({ question_level: 3 })])
    render(<WordTrainer sessionType="morning" onFinish={() => {}} />)
    await settle()

    // Lv.3 要放音频。没有声音的听音题不是「更难」，是无解
    expect(document.body.textContent).toContain('Lv.2')
    SETTINGS.tts_provider = 'edge'
  })

  it('队列为空时给出可操作的提示，而不是空白页', async () => {
    stub([])
    render(<WordTrainer sessionType="morning" onFinish={() => {}} />)
    await settle()

    expect(screen.getByText(/词库还没有可练习的词/)).toBeTruthy()
    // 空队列不该也不能开一个会话
    expect(api.startSession).not.toHaveBeenCalled()
  })
})
