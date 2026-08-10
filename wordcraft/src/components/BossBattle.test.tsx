import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, act, cleanup } from '@testing-library/react'
import BossBattle from './BossBattle'
import * as api from '../data/api'

/**
 * 魔王讨伐的两条不变量。
 *
 * 挑这两条不是为了覆盖率，是因为**变异测试证明其余门禁都拦不住它们**：
 * 删掉输入锁之后 `tsc`、`oxlint`、107 条测试全部通过，回归会静默上线。
 *
 * `BOSS_HP = 3` 的语义是「连对三次」——三次独立的回忆。若允许在选项重排的
 * 空窗里连点同一个按钮，它就退化成「同一个位置点三次」，整个魔王玩法失去意义。
 */

vi.mock('../core/sound', () => ({
  playCorrect: vi.fn(),
  playIncorrect: vi.fn(),
  playLevelUp: vi.fn(),
}))

const BOSS: api.BossWord = {
  word_id: 1,
  word: 'abandon',
  phonetic: '/əˈbændən/',
  pos: 'v',
  meaning: '放弃，遗弃',
  example_1: 'They had to abandon the car.',
  lapses: 5,
  hp: 3,
  already_defeated: false,
}

/** 每次调用返回不同的一组干扰项，好让「选项换没换」可断言 */
function distractorPools() {
  const pools = [
    ['接受', '寻找', '维持'],
    ['归还', '占据', '削减'],
    ['提交', '拒绝', '保留'],
  ]
  let n = 0
  return vi.fn(async () => pools[n++ % pools.length])
}

beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true })
  vi.spyOn(api, 'getBossWords').mockResolvedValue([BOSS])
  vi.spyOn(api, 'getDistractorPool').mockImplementation(distractorPools())
  vi.spyOn(api, 'defeatBoss').mockResolvedValue({
    word: BOSS.word,
    dropped_block: true,
    new_question_level: 4,
  })
})

afterEach(() => {
  // 显式清理。vitest 未开 globals，RTL 的自动 cleanup 不会注册，
  // 不清的话 DOM 在测试间累积，选项按钮会从 4 个变成 12 个
  cleanup()
  vi.useRealTimers()
  vi.restoreAllMocks()
})

/** 等待异步 effect 落定 */
async function settle() {
  await act(async () => {
    await Promise.resolve()
  })
}

function optionButtons() {
  return screen
    .getAllByRole('button')
    .filter((b) => /^[A-D]/.test(b.textContent ?? ''))
}

function correctButton() {
  return optionButtons().find((b) => b.textContent?.includes(BOSS.meaning))
}

/** 从「剩余 HP N/3」里读出当前血量 */
function hp() {
  const m = document.body.textContent?.match(/(\d)\/3/)
  return m ? Number(m[1]) : null
}

describe('魔王讨伐', () => {
  it('重排窗口内的第二次点击不算命中', async () => {
    render(<BossBattle onBack={() => {}} />)
    await settle()
    expect(hp()).toBe(3)

    // **必须跨 tick 点击。** 同一 tick 内的连点会被 React 批处理——
    // 三次都读到同一个 hp，即便没有锁，结果也是 2。那样测试会
    // 因为错误的理由通过，比没有测试更糟
    await act(async () => {
      correctButton()!.click()
    })
    await act(async () => {
      await vi.advanceTimersByTimeAsync(120) // 仍在 500ms 重排窗口内
    })
    await act(async () => {
      // 此刻屏上还是旧选项。没有锁的话这一下会再次命中
      const stale = optionButtons().find((b) => b.textContent?.includes(BOSS.meaning))
      stale!.click()
    })

    expect(hp()).toBe(2)
  })

  it('命中后锁住输入，直到新选项就位', async () => {
    render(<BossBattle onBack={() => {}} />)
    await settle()

    await act(async () => {
      correctButton()!.click()
    })

    // 重排窗口内：所有选项都不可点，否则旧选项还能被继续点
    expect(optionButtons().every((b) => (b as HTMLButtonElement).disabled)).toBe(true)

    await act(async () => {
      await vi.advanceTimersByTimeAsync(600)
    })

    expect(optionButtons().some((b) => !(b as HTMLButtonElement).disabled)).toBe(true)
  })

  it('每次命中后重新取一批干扰项', async () => {
    render(<BossBattle onBack={() => {}} />)
    await settle()

    const before = optionButtons().map((b) => b.textContent)

    await act(async () => {
      correctButton()!.click()
      await vi.advanceTimersByTimeAsync(600)
    })

    const after = optionButtons().map((b) => b.textContent)
    // 选项必须换过。不换的话，即便锁住了输入，
    // 用户仍然只需记住「正确答案在第几个位置」
    expect(after).not.toEqual(before)
    expect(api.getDistractorPool).toHaveBeenCalledTimes(2)
  })

  it('答错后血量回满并换一批选项', async () => {
    render(<BossBattle onBack={() => {}} />)
    await settle()

    await act(async () => {
      correctButton()!.click()
      await vi.advanceTimersByTimeAsync(600)
    })
    expect(hp()).toBe(2)

    const wrong = optionButtons().find((b) => !b.textContent?.includes(BOSS.meaning))
    await act(async () => {
      wrong!.click()
      await vi.advanceTimersByTimeAsync(900)
    })

    // 连对三次才算记住，中途断了要从头来
    expect(hp()).toBe(3)
  })

  it('取不到新干扰项时中止本场，而不是留着旧选项继续', async () => {
    render(<BossBattle onBack={() => {}} />)
    await settle()

    vi.mocked(api.getDistractorPool).mockRejectedValueOnce(new Error('后端挂了'))

    await act(async () => {
      correctButton()!.click()
      await vi.advanceTimersByTimeAsync(600)
    })

    // 静默保留旧选项等于把连点漏洞重新打开
    expect(screen.getByText(/讨伐中断/)).toBeTruthy()
    expect(screen.getByText(/后端挂了/)).toBeTruthy()
  })
})
