import { describe, it, expect } from 'vitest'
import { transition, REINFORCE_EXIT_STREAK } from './stateMachine'
import type { AppState, QuestionType } from './types'

function input(overrides: Partial<Parameters<typeof transition>[0]> = {}) {
  return transition({
    appState: 'review' as AppState,
    questionLevel: 2 as QuestionType,
    reinforceStreak: 0,
    isCorrect: true,
    reactionMs: 2000,
    questionType: 1 as QuestionType,
    stabilityAfter: 5,
    ...overrides,
  })
}

describe('答错', () => {
  it.each(['new', 'learning', 'review', 'mastered', 'reinforcing'] as AppState[])(
    '从 %s 答错一律落入强化队列',
    (appState) => {
      const r = input({ appState, isCorrect: false })
      expect(r.appState).toBe('reinforcing')
      expect(r.reinforceStreak).toBe(0)
    },
  )

  it('答错需在本次会话内重新排队', () => {
    expect(input({ isCorrect: false }).requeueInSession).toBe(true)
    expect(input({ isCorrect: true }).requeueInSession).toBe(false)
  })

  it('答错时题型退回一级用于重建，最低不低于 Lv.1', () => {
    expect(input({ isCorrect: false, questionLevel: 4 }).questionLevel).toBe(3)
    expect(input({ isCorrect: false, questionLevel: 1 }).questionLevel).toBe(1)
  })

  it('已有连续计数被答错清零', () => {
    const r = input({ appState: 'reinforcing', reinforceStreak: 1, isCorrect: false })
    expect(r.reinforceStreak).toBe(0)
  })
})

describe('强化队列离队（决议 S3）', () => {
  it('离队条件是连续 2 次而非 spec 原定的 3 次', () => {
    expect(REINFORCE_EXIT_STREAK).toBe(2)
  })

  it('连对 1 次不升级，第 2 次升级到 review', () => {
    const first = input({ appState: 'reinforcing', reinforceStreak: 0, reactionMs: 2000 })
    expect(first.appState).toBe('reinforcing')
    expect(first.reinforceStreak).toBe(1)

    const second = input({ appState: 'reinforcing', reinforceStreak: 1, reactionMs: 2000 })
    expect(second.appState).toBe('review')
    expect(second.reinforceStreak).toBe(0)
  })

  it('超过 8 秒的答对清零计数且不升级', () => {
    const r = input({ appState: 'reinforcing', reinforceStreak: 1, reactionMs: 8000 })
    expect(r.appState).toBe('reinforcing')
    expect(r.reinforceStreak).toBe(0)
  })

  it('7999ms 计入连续，8000ms 不计', () => {
    expect(
      input({ appState: 'reinforcing', reinforceStreak: 0, reactionMs: 7999 }).reinforceStreak,
    ).toBe(1)
    expect(
      input({ appState: 'reinforcing', reinforceStreak: 0, reactionMs: 8000 }).reinforceStreak,
    ).toBe(0)
  })

  it('强化中答对不提升题型等级', () => {
    const r = input({ appState: 'reinforcing', reinforceStreak: 0, questionLevel: 2 })
    expect(r.questionLevel).toBe(2)
  })
})

describe('正常晋级', () => {
  it('new 答对进入 learning，题型保持 Lv.1', () => {
    const r = input({ appState: 'new', questionLevel: 1 })
    expect(r.appState).toBe('learning')
    expect(r.questionLevel).toBe(1)
  })

  it.each(['learning', 'review'] as AppState[])(
    '%s 答对后进入 review',
    (appState) => {
      expect(input({ appState }).appState).toBe('review')
    },
  )

  it('learning / review 答对提升题型等级，封顶 Lv.5', () => {
    expect(input({ questionLevel: 1 }).questionLevel).toBe(2)
    expect(input({ questionLevel: 5 }).questionLevel).toBe(5)
  })
})

describe('掌握判定', () => {
  it('需同时满足稳定性 >60 天与高阶题型通过', () => {
    // 两条都满足
    expect(input({ stabilityAfter: 61, questionType: 4 }).appState).toBe('mastered')
    // 稳定性不足
    expect(input({ stabilityAfter: 60, questionType: 4 }).appState).toBe('review')
    // 题型不够高
    expect(input({ stabilityAfter: 100, questionType: 3 }).appState).toBe('review')
  })

  it('已掌握词抽查通过维持 mastered', () => {
    const r = input({ appState: 'mastered', stabilityAfter: 200, questionType: 2 })
    expect(r.appState).toBe('mastered')
  })

  it('已掌握词抽查失败回落强化队列', () => {
    const r = input({ appState: 'mastered', isCorrect: false })
    expect(r.appState).toBe('reinforcing')
    expect(r.requeueInSession).toBe(true)
  })
})
