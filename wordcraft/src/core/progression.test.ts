import { describe, it, expect } from 'vitest'
import { comboMultiplier, levelForXp, levelProgress, xpFor, MAX_LEVEL } from './progression'

describe('连击倍率', () => {
  it('三档边界正确', () => {
    expect(comboMultiplier(2)).toBe(1.0)
    expect(comboMultiplier(3)).toBe(1.2)
    expect(comboMultiplier(4)).toBe(1.2)
    expect(comboMultiplier(5)).toBe(1.5)
    expect(comboMultiplier(7)).toBe(1.5)
    expect(comboMultiplier(8)).toBe(2.0)
    expect(comboMultiplier(100)).toBe(2.0)
  })

  it('零连击不加成', () => {
    expect(comboMultiplier(0)).toBe(1.0)
  })
})

describe('XP 计算', () => {
  it('无连击时等于基础值', () => {
    expect(xpFor(4, 0)).toBe(15)
    expect(xpFor(3, 0)).toBe(10)
    expect(xpFor(2, 0)).toBe(5)
    expect(xpFor(1, 0)).toBe(1)
  })

  it('连击放大 XP', () => {
    expect(xpFor(4, 3)).toBe(18) // 15 × 1.2
    expect(xpFor(4, 5)).toBe(23) // 15 × 1.5 = 22.5 → 23
    expect(xpFor(4, 8)).toBe(30) // 15 × 2.0
  })
})

describe('等级公式', () => {
  it('与 Rust 侧 player_stats::level_for_xp 保持一致', () => {
    expect(levelForXp(0)).toBe(1)
    expect(levelForXp(49)).toBe(1)
    expect(levelForXp(50)).toBe(2)
    expect(levelForXp(200)).toBe(3)
    expect(levelForXp(450)).toBe(4)
  })

  it('负数与零都返回 1 级', () => {
    expect(levelForXp(-100)).toBe(1)
    expect(levelForXp(0)).toBe(1)
  })

  it('封顶 100 级', () => {
    expect(levelForXp(500_000)).toBe(MAX_LEVEL)
    expect(levelForXp(999_999_999)).toBe(MAX_LEVEL)
  })
})

describe('等级进度', () => {
  it('等级起点处进度为零', () => {
    const p = levelProgress(50)
    expect(p.level).toBe(2)
    expect(p.current).toBe(0)
    expect(p.ratio).toBe(0)
  })

  it('进度比例落在 0..1', () => {
    for (const xp of [0, 25, 50, 123, 456, 10000]) {
      const p = levelProgress(xp)
      expect(p.ratio).toBeGreaterThanOrEqual(0)
      expect(p.ratio).toBeLessThanOrEqual(1)
    }
  })

  it('满级时进度为满', () => {
    const p = levelProgress(500_000)
    expect(p.level).toBe(MAX_LEVEL)
    expect(p.ratio).toBe(1)
  })

  it('所需 XP 随等级递增', () => {
    expect(levelProgress(60).needed).toBeLessThan(levelProgress(210).needed)
  })
})
