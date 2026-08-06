import { isFastCorrect } from './rating'
import type { AppState, QuestionType } from './types'

/**
 * 业务状态机。contracts §4。
 *
 * 与 FSRS 自身的 state 分开维护（ADR-6）：ts-fsrs 的 Relearning 由算法内部的
 * lapse 触发并自行退出，而这里的 reinforcing 有产品规则——连续答对若干次才能
 * 离开、每次弹窗保底占比。把两者压进一列，早晚要写出「这里的 Relearning 其实
 * 是指…」这种注释。
 */

/** 离开强化队列所需的连续快速答对次数。 */
export const REINFORCE_EXIT_STREAK = 2

/** 判定「已掌握」的稳定性门槛（天）。 */
export const MASTERY_STABILITY_DAYS = 60

/** 判定「已掌握」所需的最低题型等级。 */
export const MASTERY_MIN_QUESTION_LEVEL = 4

export interface TransitionInput {
  appState: AppState
  questionLevel: QuestionType
  reinforceStreak: number
  isCorrect: boolean
  reactionMs: number
  questionType: QuestionType
  /** FSRS 算出的新稳定性，用于判定是否达到掌握门槛 */
  stabilityAfter: number
}

export interface TransitionResult {
  appState: AppState
  questionLevel: QuestionType
  reinforceStreak: number
  /** 是否需要在本次会话内重新排队（答错当场重考，spec F2） */
  requeueInSession: boolean
}

function clampLevel(level: number): QuestionType {
  return Math.min(Math.max(level, 1), 5) as QuestionType
}

export function transition(input: TransitionInput): TransitionResult {
  const {
    appState,
    questionLevel,
    reinforceStreak,
    isCorrect,
    reactionMs,
    stabilityAfter,
  } = input

  // 答错：任何状态都落入强化队列，计数清零，题型退回一级重建
  if (!isCorrect) {
    return {
      appState: 'reinforcing',
      questionLevel: clampLevel(questionLevel - 1),
      reinforceStreak: 0,
      requeueInSession: true,
    }
  }

  if (appState === 'reinforcing') {
    // 超过 8 秒的答对不计入连续——「想起来了」和「记住了」不是一回事
    if (!isFastCorrect(isCorrect, reactionMs)) {
      return {
        appState: 'reinforcing',
        questionLevel,
        reinforceStreak: 0,
        requeueInSession: false,
      }
    }

    const streak = reinforceStreak + 1
    if (streak >= REINFORCE_EXIT_STREAK) {
      return {
        appState: 'review',
        questionLevel,
        reinforceStreak: 0,
        requeueInSession: false,
      }
    }
    return {
      appState: 'reinforcing',
      questionLevel,
      reinforceStreak: streak,
      requeueInSession: false,
    }
  }

  // 达到掌握门槛：稳定性足够且刚通过高阶题型
  const nextLevel = clampLevel(questionLevel + 1)
  if (
    stabilityAfter > MASTERY_STABILITY_DAYS &&
    input.questionType >= MASTERY_MIN_QUESTION_LEVEL
  ) {
    return {
      appState: 'mastered',
      questionLevel: nextLevel,
      reinforceStreak: 0,
      requeueInSession: false,
    }
  }

  // new / learning / review / mastered 答对后统一进入 review
  // （mastered 词答对属于低频抽查通过，维持已掌握）
  return {
    appState: appState === 'mastered' ? 'mastered' : 'review',
    questionLevel: nextLevel,
    reinforceStreak: 0,
    requeueInSession: false,
  }
}
