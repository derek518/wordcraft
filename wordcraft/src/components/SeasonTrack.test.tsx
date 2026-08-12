import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, act, cleanup } from '@testing-library/react'
import SeasonTrack from './SeasonTrack'
import * as api from '../data/api'

/**
 * 赛季赛道的两条不变量。
 *
 * 一是**里程碑积分必须来自后端**。写死的数字会在改价后悄悄变成谎话——
 * 蓝图描述（写 28 实为 34）、词库总数（写死 3,657）都栽在同一件事上，
 * 这是第三次，所以给它钉一颗钉子。
 *
 * 二是**幽灵车不能盖住自己的进度**。改版前两台车共用一条道，幽灵的半透明灰
 * 在 DOM 里排在后面，落后于上周时会把自己整条进度涂灰——恰恰在最需要
 * 鼓励的时刻。
 */

vi.mock('../core/sound', () => ({
  playCorrect: vi.fn(),
  playLevelUp: vi.fn(),
}))

function season(over: Partial<api.SeasonState> = {}): api.SeasonState {
  return {
    week_start: '2026-08-03',
    week_end: '2026-08-09',
    sessions_done: 12,
    sessions_total: 21,
    progress: 12 / 21,
    ghost_progress: 9 / 21,
    ghost_sessions: 9,
    projected_points: 145,
    track_points: 320,
    points_per_session: 10,
    perfect_bonus: 50,
    ...over,
  }
}

async function mount(state: api.SeasonState) {
  vi.spyOn(api, 'getSeason').mockResolvedValue(state)
  render(<SeasonTrack onBack={() => {}} />)
  await act(async () => {
    await Promise.resolve()
  })
}

/** 整页可见文本。用于断言被拆在多个元素里的内容 */
const text = () => document.body.textContent ?? ''

beforeEach(() => vi.restoreAllMocks())
afterEach(cleanup)

describe('赛季赛道', () => {
  it('里程碑积分按后端参数计算，不是写死的', async () => {
    await mount(season())

    // 数字被拆在 <span> 里，getByText 不跨元素匹配，故直接看整页文本
    // 3/7/14 档 = 时段 × 每时段分；21 档另加完美周奖励
    for (const [sessions, points] of [[3, 30], [7, 70], [14, 140], [21, 260]]) {
      expect(text()).toContain(`${sessions}时段 · ${points}分`)
    }
  })

  it('改动后端计分参数，界面数字随之改变', async () => {
    await mount(season({ points_per_session: 20, perfect_bonus: 100 }))

    // 若前端写死 10 分/时段，这里仍会显示 30 而不是 60
    expect(text()).toContain('3时段 · 60分')
    expect(text()).toContain('21时段 · 520分')
  })

  it('本周与上周是两条独立的赛道', async () => {
    await mount(season())

    // 合成一条时两台车会在接近处叠成一团，而跟自己比接近才是常态
    expect(screen.getByText('本周')).toBeTruthy()
    expect(screen.getByText('上周')).toBeTruthy()
  })

  it('落后于上周时，本周进度条不被幽灵覆盖', async () => {
    await mount(season({ sessions_done: 5, progress: 5 / 21, ghost_sessions: 15, ghost_progress: 15 / 21 }))

    // 宽度写成 calc(<pct>% - 8px)，按百分比数值定位
    const bars = [...document.querySelectorAll<HTMLElement>('[style]')].filter((el) =>
      el.style.width.includes('%'),
    )
    const pct = (n: number) => `${(n / 21) * 100}`
    const mine = bars.find((el) => el.style.width.includes(pct(5)))
    const ghost = bars.find((el) => el.style.width.includes(pct(15)))
    expect(mine).toBeTruthy()
    expect(ghost).toBeTruthy()

    // 两条填充必须在不同的容器里。同属一个容器时后画的会盖住先画的，
    // 落后时自己的进度就整条变灰
    expect(mine!.parentElement).not.toBe(ghost!.parentElement)
  })

  it('未达成的里程碑刻度不可点击', async () => {
    await mount(season({ sessions_done: 1, progress: 1 / 21 }))

    // 四个刻度原先都是无 disabled 的按钮，onClick 里靠 `reached &&` 静默返回。
    // 光标是手型、有 hover 效果，点下去却毫无反应——用户会一个个点过去等结果
    const marks = [...document.querySelectorAll('button')].filter((b) =>
      b.querySelector('img[src*="medal"], img[src*="crown"]'),
    )
    expect(marks.length).toBe(4)
    expect(marks.every((b) => (b as HTMLButtonElement).disabled)).toBe(true)

    // 悬停要能说明为什么点不了
    expect(marks[0].getAttribute('title')).toMatch(/还差 2 个时段/)
  })

  it('达成的里程碑刻度可以点开庆祝', async () => {
    await mount(season({ sessions_done: 8, progress: 8 / 21 }))

    const marks = [...document.querySelectorAll('button')].filter((b) =>
      b.querySelector('img[src*="medal"], img[src*="crown"]'),
    )
    // 3 与 7 已达成，14 与 21 未达成
    expect(marks.filter((b) => !(b as HTMLButtonElement).disabled).length).toBe(2)
    expect(marks[0].getAttribute('title')).toMatch(/已达成/)
  })

  it('起止日期取自后端，前端不自己算日历', async () => {
    await mount(season({ week_start: '2026-12-28', week_end: '2027-01-03' }))

    // 跨年跨月。前端若自己加六天，时区解析还会整体偏一天
    expect(screen.getByText('12/28 - 1/3')).toBeTruthy()
  })
})
