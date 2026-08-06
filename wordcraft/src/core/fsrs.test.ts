import { describe, it, expect } from 'vitest'
import { gradeAnswer, toCard } from './fsrs'
import type { QueueItem } from './types'

function newWord(overrides: Partial<QueueItem> = {}): QueueItem {
  return {
    word_id: 1,
    word: 'crystal',
    phonetic: '/ˈkrɪstl/',
    pos: 'n.',
    meaning: '水晶',
    example_1: 'A glowing crystal.',
    example_2: '',
    difficulty: 0,
    stability: 0,
    due_at: null,
    fsrs_state: 0,
    app_state: 'new',
    reps: 0,
    lapses: 0,
    question_level: 1,
    reinforce_streak: 0,
    source: 'new',
    ...overrides,
  }
}

describe('Card 还原', () => {
  it('新词还原为空卡', () => {
    const card = toCard(newWord())
    expect(card.reps).toBe(0)
    expect(card.state).toBe(0)
    expect(card.stability).toBe(0)
  })

  it('已学词保留原有 FSRS 状态', () => {
    const card = toCard(
      newWord({
        reps: 3,
        lapses: 1,
        stability: 12.5,
        difficulty: 6.2,
        fsrs_state: 2,
        app_state: 'review',
        due_at: '2026-08-10T00:00:00Z',
      }),
    )
    expect(card.reps).toBe(3)
    expect(card.lapses).toBe(1)
    expect(card.stability).toBeCloseTo(12.5)
    expect(card.difficulty).toBeCloseTo(6.2)
    expect(card.state).toBe(2)
  })
})

describe('完整评分流程', () => {
  it('ts-fsrs 输出被完整映射，无字段丢失', () => {
    const { dto } = gradeAnswer({
      item: newWord(),
      questionType: 1,
      isCorrect: true,
      reactionMs: 2000,
      sessionId: 7,
    })

    expect(dto.wordId).toBe(1)
    expect(dto.sessionId).toBe(7)
    expect(dto.questionType).toBe(1)
    expect(dto.isCorrect).toBe(true)
    expect(dto.reactionMs).toBe(2000)
    expect(dto.rating).toBe(4) // 2000ms < 3000ms → Easy

    // before 取自入参
    expect(dto.before.difficulty).toBe(0)
    expect(dto.before.stability).toBe(0)

    // after 全部来自 ts-fsrs，且都是有效数值
    expect(dto.after.difficulty).toBeGreaterThan(0)
    expect(dto.after.stability).toBeGreaterThan(0)
    expect(dto.after.reps).toBe(1)
    expect(dto.after.lapses).toBe(0)
    expect([0, 1, 2, 3]).toContain(dto.after.fsrsState)
  })

  it('dueAt 是契约要求的 UTC ISO8601 且不含毫秒', () => {
    const { dto } = gradeAnswer({
      item: newWord(),
      questionType: 1,
      isCorrect: true,
      reactionMs: 2000,
      sessionId: null,
    })
    expect(dto.after.dueAt).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/)
    expect(Number.isNaN(Date.parse(dto.after.dueAt))).toBe(false)
  })

  it('答对使到期日推后', () => {
    const now = new Date('2026-08-06T10:00:00Z')
    const { dto } = gradeAnswer({
      item: newWord(),
      questionType: 1,
      isCorrect: true,
      reactionMs: 2000,
      sessionId: null,
      now,
    })
    expect(Date.parse(dto.after.dueAt)).toBeGreaterThan(now.getTime())
  })

  it('答错时评级与状态一致，且要求会话内重排', () => {
    const result = gradeAnswer({
      // fsrs_state 必须是 Review(2)：ts-fsrs 只在 Review/Relearning 阶段答错才计 lapse，
      // New 阶段答错不算遗忘。app_state 与 fsrs_state 语义不同（ADR-6）但不能任意组合
      item: newWord({
        app_state: 'review',
        fsrs_state: 2,
        reps: 3,
        stability: 20,
        difficulty: 5,
        due_at: '2026-08-01T00:00:00Z',
      }),
      questionType: 1,
      isCorrect: false,
      reactionMs: 9000,
      sessionId: null,
    })

    expect(result.dto.rating).toBe(1)
    expect(result.dto.appState).toBe('reinforcing')
    expect(result.requeueInSession).toBe(true)
    expect(result.dto.after.lapses).toBe(1)
  })

  it('新词阶段答错不计入遗忘次数', () => {
    const result = gradeAnswer({
      item: newWord(), // fsrs_state = 0 (New)
      questionType: 1,
      isCorrect: false,
      reactionMs: 9000,
      sessionId: null,
    })
    // FSRS 的 lapse 语义是「已记住的东西又忘了」，从未记住过就不算
    expect(result.dto.after.lapses).toBe(0)
    expect(result.dto.appState).toBe('reinforcing')
  })

  it('载荷满足后端校验：答对不评 Again、答错必评 Again', () => {
    const correct = gradeAnswer({
      item: newWord(),
      questionType: 1,
      isCorrect: true,
      reactionMs: 15000,
      sessionId: null,
    })
    expect(correct.dto.rating).not.toBe(1)

    const wrong = gradeAnswer({
      item: newWord(),
      questionType: 1,
      isCorrect: false,
      reactionMs: 1000,
      sessionId: null,
    })
    expect(wrong.dto.rating).toBe(1)
  })

  describe('强化队列到期日覆盖（contracts §4.2）', () => {
    it('强化中的词到期日不超过次日，即便 FSRS 给了长间隔', () => {
      const now = new Date('2026-08-06T10:00:00Z')
      // 错过一次、刚答对一次：streak 未达 2，仍在强化队列。
      // 此时 Easy 评级会让 FSRS 给出多天间隔
      const { dto } = gradeAnswer({
        item: newWord({
          app_state: 'reinforcing',
          fsrs_state: 3,
          reps: 2,
          lapses: 1,
          stability: 6,
          difficulty: 5,
          reinforce_streak: 0,
        }),
        questionType: 1,
        isCorrect: true,
        reactionMs: 1500, // Easy
        sessionId: null,
        now,
      })

      expect(dto.appState).toBe('reinforcing')
      const dueMs = Date.parse(dto.after.dueAt)
      const limit = now.getTime() + 24 * 60 * 60 * 1000
      expect(dueMs).toBeLessThanOrEqual(limit)
    })

    it('非强化状态不受此上限约束', () => {
      const now = new Date('2026-08-06T10:00:00Z')
      const { dto } = gradeAnswer({
        item: newWord({
          app_state: 'review',
          fsrs_state: 2,
          reps: 5,
          stability: 40,
          difficulty: 4,
          due_at: '2026-08-06T00:00:00Z',
        }),
        questionType: 1,
        isCorrect: true,
        reactionMs: 1500,
        sessionId: null,
        now,
      })

      expect(dto.appState).toBe('review')
      // 复习词应享受 FSRS 的长间隔
      const dueMs = Date.parse(dto.after.dueAt)
      expect(dueMs).toBeGreaterThan(now.getTime() + 24 * 60 * 60 * 1000)
    })

    it('答错落入强化队列时到期日也受限', () => {
      const now = new Date('2026-08-06T10:00:00Z')
      const { dto } = gradeAnswer({
        item: newWord({
          app_state: 'review',
          fsrs_state: 2,
          reps: 3,
          stability: 30,
          difficulty: 5,
        }),
        questionType: 1,
        isCorrect: false,
        reactionMs: 9000,
        sessionId: null,
        now,
      })
      expect(dto.appState).toBe('reinforcing')
      expect(Date.parse(dto.after.dueAt)).toBeLessThanOrEqual(
        now.getTime() + 24 * 60 * 60 * 1000,
      )
    })
  })

  it('stability 有值而 difficulty 为 0 时不崩溃', () => {
    // 摸底预分级（T28）只赋 stability 就会产生这种状态。ts-fsrs 对此抛
    // FSRSValidationError，若不兜底会让整个会话在运行时挂掉
    expect(() =>
      gradeAnswer({
        item: newWord({ app_state: 'review', fsrs_state: 2, reps: 1, stability: 14, difficulty: 0 }),
        questionType: 1,
        isCorrect: true,
        reactionMs: 2000,
        sessionId: null,
      }),
    ).not.toThrow()
  })

  it('强化中的词连对两次后离开强化队列', () => {
    const item = newWord({
      app_state: 'reinforcing',
      reps: 2,
      lapses: 1,
      stability: 1,
      difficulty: 7,
      reinforce_streak: 1,
      fsrs_state: 3,
    })
    const { dto } = gradeAnswer({
      item,
      questionType: 1,
      isCorrect: true,
      reactionMs: 3000,
      sessionId: null,
    })
    expect(dto.appState).toBe('review')
    expect(dto.reinforceStreak).toBe(0)
  })

  it('所有数值字段非负，满足后端校验', () => {
    for (const isCorrect of [true, false]) {
      for (const reactionMs of [500, 5000, 30000]) {
        const { dto } = gradeAnswer({
          item: newWord({ reps: 5, lapses: 2, stability: 30, difficulty: 6 }),
          questionType: 3,
          isCorrect,
          reactionMs,
          sessionId: null,
        })
        expect(dto.after.stability).toBeGreaterThanOrEqual(0)
        expect(dto.after.difficulty).toBeGreaterThanOrEqual(0)
        expect(dto.after.reps).toBeGreaterThanOrEqual(0)
        expect(dto.after.lapses).toBeGreaterThanOrEqual(0)
        expect(dto.reactionMs).toBeGreaterThanOrEqual(0)
      }
    }
  })
})
