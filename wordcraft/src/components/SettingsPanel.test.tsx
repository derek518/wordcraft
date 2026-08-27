import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, act, cleanup } from '@testing-library/react'
import SettingsPanel from './SettingsPanel'
import * as api from '../data/api'

vi.mock('../core/sound', () => ({ playCorrect: vi.fn(), setSoundEnabled: vi.fn() }))

const VALUES: Record<string, string> = {
  session_windows: '09:00-11:00,13:00-15:00,19:00-21:00',
  daily_new_words: '6',
  session_word_count: '20',
  sound_enabled: 'true',
  tts_provider: 'edge',
  autostart_enabled: 'true',
  study_level: 'senior',
  study_days: '1,2,3,4,5,6,7',
}

const STATS = {
  total_words: 2076, untouched: 2068, total_reviews: 389,
  total_xp: 5486, level: 11, current_streak: 0, best_streak: 12,
  vocab_estimate: 1382, draw_tickets: 0, makeup_cards: 0,
}

const LEVELS: api.StudyLevelOption[] = [
  { value: 'junior', label: '初中', words: 1581 },
  { value: 'senior', label: '高中', words: 2076 },
  { value: 'all', label: '全部', words: 3657 },
]

function stub() {
  vi.spyOn(api, 'getStudyLevels').mockResolvedValue(LEVELS)
  vi.spyOn(api, 'getOverallStats').mockResolvedValue(STATS)
  vi.spyOn(api, 'getSetting').mockImplementation(async (k) => VALUES[k] ?? null)
  vi.spyOn(api, 'setAutostart').mockResolvedValue(undefined)
  vi.spyOn(api, 'exportDataJson').mockResolvedValue('{}')
  return vi.spyOn(api, 'setSetting').mockResolvedValue(undefined)
}

const btn = (t: string) =>
  [...document.querySelectorAll('button')].find((b) => b.textContent?.includes(t))

const text = () => document.body.textContent ?? ''

async function settle() {
  for (let i = 0; i < 3; i++) await act(async () => { await Promise.resolve() })
}

beforeEach(() => vi.restoreAllMocks())
afterEach(cleanup)

describe('设置面板', () => {
  it('保存后必定回读一次，不采信自己发出去的值', async () => {
    const set = stub()
    const get = vi.spyOn(api, 'getSetting').mockImplementation(async (k) => VALUES[k] ?? null)
    render(<SettingsPanel onBack={() => {}} />)
    await settle()
    const before = get.mock.calls.filter((c) => c[0] === 'tts_provider').length

    await act(async () => {
      btn('关闭发音')!.click()
    })
    await settle()

    expect(set).toHaveBeenCalledWith('tts_provider', 'off')
    // 后端有权规范化或拒绝写入。直接采信提交值，界面会与实际存储分叉
    const after = get.mock.calls.filter((c) => c[0] === 'tts_provider').length
    expect(after).toBeGreaterThan(before)
  })

  it('保存失败时报错并重新载入，不留下与后端不符的界面', async () => {
    stub()
    vi.spyOn(api, 'setSetting').mockRejectedValue(new Error('磁盘只读'))
    const get = vi.spyOn(api, 'getSetting').mockImplementation(async (k) => VALUES[k] ?? null)

    render(<SettingsPanel onBack={() => {}} />)
    await settle()
    const before = get.mock.calls.length

    await act(async () => {
      btn('关闭发音')!.click()
    })
    await settle()

    expect(screen.getByText(/磁盘只读/)).toBeTruthy()
    // 失败后要重新拉一次，把界面拉回后端的真实状态
    expect(get.mock.calls.length).toBeGreaterThan(before)
  })

  it('全部设置项都从后端读，界面不带默认值', async () => {
    const get = vi.spyOn(api, 'getSetting').mockImplementation(async (k) => VALUES[k] ?? null)
    vi.spyOn(api, 'setSetting').mockResolvedValue(undefined)

    render(<SettingsPanel onBack={() => {}} />)
    await settle()

    // 契约 §2.1 的键；前端写死默认值会在后端改默认时静静分叉
    const asked = get.mock.calls.map((c) => c[0])
    for (const k of ['session_windows', 'daily_new_words', 'session_word_count', 'tts_provider', 'autostart_enabled', 'study_level', 'study_days']) {
      expect(asked).toContain(k)
    }
  })

  it('开机自启走 set_autostart，不只写 settings 键', async () => {
    stub()
    const auto = vi.spyOn(api, 'setAutostart').mockResolvedValue(undefined)
    render(<SettingsPanel onBack={() => {}} />)
    await settle()

    await act(async () => {
      document.querySelector<HTMLButtonElement>('[aria-label="开机自启"]')!.click()
    })
    await settle()

    expect(auto).toHaveBeenCalledWith(false)
    expect(api.setSetting).not.toHaveBeenCalledWith('autostart_enabled', expect.anything())
  })

  it('学习范围默认高中，切换写入 study_level', async () => {
    const set = stub()
    render(<SettingsPanel onBack={() => {}} />)
    await settle()

    // 默认必须是高中：默认错了，用户要几个月后才会发现自己在背虚词
    const senior = btn('高中')!
    expect(senior.className).toContain('border-wc-primary')

    await act(async () => {
      btn('初中')!.click()
    })
    await settle()
    expect(set).toHaveBeenCalledWith('study_level', 'junior')
  })

  it('范围选项与词数来自后端，不写死', async () => {
    stub()
    vi.spyOn(api, 'getStudyLevels').mockResolvedValue([
      { value: 'senior', label: '高中', words: 2100 },
      { value: 'cet4', label: '四级', words: 1800 },
    ])
    render(<SettingsPanel onBack={() => {}} />)
    await settle()

    // 四级词导入后选项应自动出现，词数也照库里的算——
    // 写死的计数在本项目已三次变成谎话
    expect(text()).toContain('四级')
    expect(text()).toContain('1800')
    expect(text()).toContain('2100')
  })

  it('取消工作日后只剩周末', async () => {
    const set = stub()
    render(<SettingsPanel onBack={() => {}} />)
    await settle()

    await act(async () => {
      btn('五')!.click()
    })
    await settle()
    expect(set).toHaveBeenCalledWith('study_days', '1,2,3,4,6,7')
  })

  it('不允许把学习日清空', async () => {
    const set = stub()
    vi.spyOn(api, 'getSetting').mockImplementation(async (k) =>
      k === 'study_days' ? '6' : VALUES[k] ?? null,
    )
    render(<SettingsPanel onBack={() => {}} />)
    await settle()

    await act(async () => {
      btn('六')!.click() // 取消掉唯一剩下的那天
    })
    await settle()

    // 一天都不学等于停用应用。后端会静静回落到「每天」，
    // 那种「点了没反应还悄悄变回去」比直接不允许更难理解
    expect(set).not.toHaveBeenCalledWith('study_days', expect.anything())
  })

  it('按学习日与每场新词估算走完剩余生词的周数', async () => {
    stub()
    render(<SettingsPanel onBack={() => {}} />)
    await settle()

    // 7 天 × 3 场 × 6 新词 = 126/周，2068 个生词约 17 周
    expect(text()).toContain('2068')
    expect(text()).toContain('126')
    expect(text()).toContain('17')
  })

  it('周末两天时给出超过半年的提醒', async () => {
    vi.spyOn(api, 'getStudyLevels').mockResolvedValue(LEVELS)
    vi.spyOn(api, 'getOverallStats').mockResolvedValue(STATS)
    vi.spyOn(api, 'setSetting').mockResolvedValue(undefined)
    vi.spyOn(api, 'setAutostart').mockResolvedValue(undefined)
    vi.spyOn(api, 'exportDataJson').mockResolvedValue('{}')
    vi.spyOn(api, 'getSetting').mockImplementation(async (k) =>
      k === 'study_days' ? '6,7' : VALUES[k] ?? null,
    )
    render(<SettingsPanel onBack={() => {}} />)
    await settle()

    // 2 天 × 3 场 × 6 = 36/周 → 58 周。默认配置是按「每天都能用」调的，
    // 改成只有周末之后界面上看不出任何区别，这条提醒就是那个区别
    expect(text()).toContain('58')
    expect(text()).toContain('超过半年')
  })

  it('导出按钮真正请求后端，失败时显示原因', async () => {
    stub()
    vi.spyOn(api, 'exportDataJson').mockRejectedValue(new Error('磁盘只读'))
    render(<SettingsPanel onBack={() => {}} />)
    await settle()

    await act(async () => {
      btn('导出 JSON')!.click()
    })
    await settle()

    expect(screen.getByText(/磁盘只读/)).toBeTruthy()
  })
})
