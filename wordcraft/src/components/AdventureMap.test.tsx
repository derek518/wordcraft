import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, act, cleanup } from '@testing-library/react'
import AdventureMap from './AdventureMap'
import * as api from '../data/api'
import type { OverallStats } from '../core/types'

vi.mock('../core/sound', () => ({ playCorrect: vi.fn(), playLevelUp: vi.fn() }))

function zone(key: string, name: string, unlocked: boolean, learned = 0, total = 600) {
  return { key, name, total, learned, unlocked }
}

const STATS: OverallStats = {
  total_words: 3657, untouched: 3000, total_reviews: 100,
  total_xp: 500, level: 5, current_streak: 3, best_streak: 7,
  vocab_estimate: 800, draw_tickets: 2, makeup_cards: 1,
}

function stub(zones = [zone('newbie', '新手村', true, 42, 300), zone('grass', '清风平原', false)]) {
  vi.spyOn(api, 'getTodaySessions').mockResolvedValue([])
  vi.spyOn(api, 'getZoneProgress').mockResolvedValue(zones as never)
  vi.spyOn(api, 'getSetting').mockResolvedValue('edge')
}

async function settle() {
  for (let i = 0; i < 3; i++) await act(async () => { await Promise.resolve() })
}

const btn = (t: string) =>
  [...document.querySelectorAll('button')].find((b) => b.textContent?.includes(t))

beforeEach(() => vi.restoreAllMocks())
afterEach(cleanup)

describe('冒险地图', () => {
  it('区域解锁状态取自后端，不由前端判定', async () => {
    stub()
    render(<AdventureMap onStartTraining={() => {}} onOpenStats={() => {}} onOpenAlbum={() => {}}
      onOpenHomestead={() => {}} onOpenSeason={() => {}} onOpenBoss={() => {}}
      onOpenLibrary={() => {}} stats={STATS} />)
    await settle()

    // 这里曾把 unlocked 写成硬编码的 true，六个区域里五个词数也是错的——
    // 用户点亮 50 个水晶仍停在新手村，而界面看不出任何异常
    expect(document.body.textContent).toContain('新手村')
    expect(document.body.textContent).toContain('清风平原')
    expect(document.body.textContent).toMatch(/Lv\.\s*解锁/)
  })

  it('区域词数用后端返回的值', async () => {
    stub([zone('newbie', '新手村', true, 42, 317)])
    render(<AdventureMap onStartTraining={() => {}} onOpenStats={() => {}} onOpenAlbum={() => {}}
      onOpenHomestead={() => {}} onOpenSeason={() => {}} onOpenBoss={() => {}}
      onOpenLibrary={() => {}} stats={STATS} />)
    await settle()

    // 写死的词数会在词库更新后变成谎话，这是本项目栽过三次的同一件事
    expect(document.body.textContent).toContain('317')
  })

  it('自由探险先问练哪一种，不直接开练', async () => {
    const start = vi.fn()
    stub()
    render(<AdventureMap onStartTraining={start} onOpenStats={() => {}} onOpenAlbum={() => {}}
      onOpenHomestead={() => {}} onOpenSeason={() => {}} onOpenBoss={() => {}}
      onOpenLibrary={() => {}} stats={STATS} />)
    await settle()

    await act(async () => {
      btn('自由探险')!.click()
    })

    expect(start).not.toHaveBeenCalled()
    expect(document.body.textContent).toContain('拼写专项')
    expect(document.body.textContent).toContain('听写模式')
  })

  it('关闭 TTS 时听写模式不可选，并说明原因', async () => {
    stub()
    vi.spyOn(api, 'getSetting').mockResolvedValue('off')
    render(<AdventureMap onStartTraining={() => {}} onOpenStats={() => {}} onOpenAlbum={() => {}}
      onOpenHomestead={() => {}} onOpenSeason={() => {}} onOpenBoss={() => {}}
      onOpenLibrary={() => {}} stats={STATS} />)
    await settle()

    await act(async () => {
      btn('自由探险')!.click()
    })

    // 没有声音的听写不是「更难」，是无解。灰掉还要说清为什么
    expect((btn('听写模式') as HTMLButtonElement).disabled).toBe(true)
    expect(document.body.textContent).toContain('需要先在设置里开启发音')
  })

  it('选定模式后带着模式启动，而不是普通自由练习', async () => {
    const start = vi.fn()
    stub()
    render(<AdventureMap onStartTraining={start} onOpenStats={() => {}} onOpenAlbum={() => {}}
      onOpenHomestead={() => {}} onOpenSeason={() => {}} onOpenBoss={() => {}}
      onOpenLibrary={() => {}} stats={STATS} />)
    await settle()

    await act(async () => {
      btn('自由探险')!.click()
    })
    await act(async () => {
      btn('拼写专项')!.click()
    })

    expect(start).toHaveBeenCalledWith('free', 'spelling')
  })

  it('加载失败显示原因而不是假装今天没有时段', async () => {
    stub()
    vi.spyOn(api, 'getTodaySessions').mockRejectedValue(new Error('数据库锁失败'))
    render(<AdventureMap onStartTraining={() => {}} onOpenStats={() => {}} onOpenAlbum={() => {}}
      onOpenHomestead={() => {}} onOpenSeason={() => {}} onOpenBoss={() => {}}
      onOpenLibrary={() => {}} stats={STATS} />)
    await settle()

    expect(document.body.textContent).toContain('数据库锁失败')
    expect(btn('重试')).toBeTruthy()
  })

  it('磐石秘境使用自己的元素，不套用新手村文案', async () => {
    stub([zone('rock', '磐石秘境', true, 12, 457)])
    render(<AdventureMap onStartTraining={() => {}} onOpenStats={() => {}} onOpenAlbum={() => {}}
      onOpenHomestead={() => {}} onOpenSeason={() => {}} onOpenBoss={() => {}}
      onOpenLibrary={() => {}} stats={STATS} />)
    await settle()

    expect(document.body.textContent).toContain('磐石秘境')
    expect(document.body.textContent).toContain('低频难词')
    expect(document.body.textContent).not.toContain('冒险的起点')
  })
})
