import type { Rating } from './types'

/**
 * XP 与等级。contracts §7。
 *
 * 等级公式与 Rust 侧 `player_stats::level_for_xp` 必须一致——前端用于即时反馈，
 * 后端用于持久化，两者算出不同数字会让用户看到等级来回跳。
 */

export const BASE_XP: Record<Rating, number> = {
  1: 1, // Again
  2: 5, // Hard
  3: 10, // Good
  4: 15, // Easy
}

export const MAX_LEVEL = 100

/**
 * 连击倍率。
 *
 * 连击是即时反馈的核心——它让「连续答对」本身成为可追求的目标，
 * 而不必等到会话结束才有奖励。
 */
export function comboMultiplier(combo: number): number {
  if (combo >= 8) return 2.0
  if (combo >= 5) return 1.5
  if (combo >= 3) return 1.2
  return 1.0
}

/** `combo` 为本次答对之前已连对的次数。 */
export function xpFor(rating: Rating, combo: number): number {
  return Math.round(BASE_XP[rating] * comboMultiplier(combo))
}

export function levelForXp(totalXp: number): number {
  if (totalXp <= 0) return 1
  return Math.min(Math.floor(Math.sqrt(totalXp / 50)) + 1, MAX_LEVEL)
}

/** 当前等级内的进度，用于渲染经验条。 */
export function levelProgress(totalXp: number): {
  level: number
  current: number
  needed: number
  ratio: number
} {
  const level = levelForXp(totalXp)
  if (level >= MAX_LEVEL) {
    return { level, current: 0, needed: 0, ratio: 1 }
  }
  const floorXp = 50 * (level - 1) ** 2
  const ceilXp = 50 * level ** 2
  const current = totalXp - floorXp
  const needed = ceilXp - floorXp
  return { level, current, needed, ratio: current / needed }
}
