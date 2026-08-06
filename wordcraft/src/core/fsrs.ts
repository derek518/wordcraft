import { createEmptyCard, fsrs, generatorParameters, type Card, type Grade } from 'ts-fsrs'
import { autoRate } from './rating'
import { transition } from './stateMachine'
import type { QueueItem, QuestionType, ReviewCommitDto } from './types'

/**
 * ts-fsrs 适配层。ADR-2：间隔计算在前端，Rust 只做持久化。
 *
 * 本模块是唯一接触 ts-fsrs 的地方——其余代码只看见 `ReviewCommitDto`。
 * 这样将来若要换算法（或把计算挪回 Rust），改动范围限于此文件。
 */

/**
 * 起步用 ts-fsrs 默认权重（spec §5）。
 *
 * `enable_fuzz` 打开：给到期日加入小幅随机抖动，避免同一天学的词日后总是
 * 同一天集中到期，形成复习洪峰。
 */
const scheduler = fsrs(
  generatorParameters({
    enable_fuzz: true,
    enable_short_term: true,
  }),
)

/** 把队列项还原成 ts-fsrs 的 Card。新词返回空卡。 */
export function toCard(item: QueueItem, now: Date = new Date()): Card {
  if (item.reps === 0 && item.app_state === 'new') {
    return createEmptyCard(now)
  }
  return {
    due: item.due_at ? new Date(item.due_at) : now,
    stability: item.stability,
    difficulty: item.difficulty,
    elapsed_days: 0,
    scheduled_days: 0,
    learning_steps: 0,
    reps: item.reps,
    lapses: item.lapses,
    state: item.fsrs_state,
    last_review: undefined,
  }
}

function toIsoUtc(date: Date): string {
  // 契约要求 'YYYY-MM-DDTHH:MM:SSZ'（ADR-5），toISOString 会带毫秒，需裁掉
  return `${date.toISOString().slice(0, 19)}Z`
}

export interface GradeInput {
  item: QueueItem
  questionType: QuestionType
  isCorrect: boolean
  reactionMs: number
  sessionId: number | null
  now?: Date
}

export interface GradeOutput {
  dto: ReviewCommitDto
  /** 答错需在本次会话内重新排队（spec F2） */
  requeueInSession: boolean
}

/**
 * 完整评分流程：自动评级 → FSRS 调度 → 业务状态转移 → 组装提交载荷。
 *
 * 三个环节的顺序不能换：状态机需要 FSRS 算出的新 stability 才能判定是否达到
 * 掌握门槛。
 */
export function gradeAnswer(input: GradeInput): GradeOutput {
  const { item, questionType, isCorrect, reactionMs, sessionId } = input
  const now = input.now ?? new Date()

  const rating = autoRate(isCorrect, reactionMs, questionType)
  const card = toCard(item, now)
  const { card: next } = scheduler.next(card, now, rating as Grade)

  const state = transition({
    appState: item.app_state,
    questionLevel: item.question_level,
    reinforceStreak: item.reinforce_streak,
    isCorrect,
    reactionMs,
    questionType,
    stabilityAfter: next.stability,
  })

  return {
    requeueInSession: state.requeueInSession,
    dto: {
      wordId: item.word_id,
      sessionId,
      questionType,
      isCorrect,
      reactionMs,
      rating,
      before: {
        difficulty: item.difficulty,
        stability: item.stability,
      },
      after: {
        difficulty: next.difficulty,
        stability: next.stability,
        dueAt: toIsoUtc(next.due),
        fsrsState: next.state,
        reps: next.reps,
        lapses: next.lapses,
      },
      appState: state.appState,
      questionLevel: state.questionLevel,
      reinforceStreak: state.reinforceStreak,
    },
  }
}
