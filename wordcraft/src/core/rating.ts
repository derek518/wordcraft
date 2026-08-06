import type { QuestionType, Rating } from './types'

/**
 * 自动评级。contracts §5。
 *
 * spec F2 明确禁止 Anki 式用户自评——让学习者自己判断「我记住了吗」既不准确
 * 又增加认知负担，对 ADHD 用户尤其如此。评级由正误与反应时间自动推导。
 */

/**
 * 各题型的反应时间阈值（毫秒）。
 *
 * 分题型定义是决议 S5 的结论：绝对阈值对输入型题目不成立。`perspective` 光打字
 * 就要 3-5 秒，用 Lv.1 的 3000/8000 判定会让完全掌握该词的人永远拿不到 Easy，
 * 甚至被判 Hard 而缩短间隔 —— 已掌握的词反而反复重现。
 */
export const THRESHOLDS: Record<QuestionType, { fast: number; slow: number }> = {
  1: { fast: 3000, slow: 8000 },   // 英→中 四选一
  2: { fast: 3500, slow: 9000 },   // 中→英 四选一
  3: { fast: 4000, slow: 10000 },  // 听音辨词（计时从音频播放结束起）
  4: { fast: 5000, slow: 12000 },  // 例句挖空
  5: { fast: 8000, slow: 20000 },  // 全拼写
}

/** 高阶题型答对时 rating 上调一档的起始题型 */
const ADVANCED_QUESTION_TYPE = 4

export function autoRate(
  isCorrect: boolean,
  reactionMs: number,
  questionType: QuestionType,
): Rating {
  if (!isCorrect) return 1 // Again

  const { fast, slow } = THRESHOLDS[questionType]
  let rating: Rating = reactionMs < fast ? 4 : reactionMs < slow ? 3 : 2

  // 高阶题型答对含金量更高：例句挖空与拼写考查的是产出性掌握，
  // 同样的反应时间应对应更高的评级
  if (questionType >= ADVANCED_QUESTION_TYPE) {
    rating = Math.min(rating + 1, 4) as Rating
  }

  return rating
}

/**
 * 是否算作「快速答对」——强化队列的离队判据（contracts §4）。
 *
 * 固定用 8 秒而非题型阈值：spec F2 的原文就是「连续 N 次在 8 秒内答对」，
 * 这是产品规则而非算法参数。
 */
export const FAST_ANSWER_MS = 8000

export function isFastCorrect(isCorrect: boolean, reactionMs: number): boolean {
  return isCorrect && reactionMs < FAST_ANSWER_MS
}
