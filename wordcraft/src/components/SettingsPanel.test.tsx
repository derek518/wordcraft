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
}

function stub() {
  vi.spyOn(api, 'getSetting').mockImplementation(async (k) => VALUES[k] ?? null)
  vi.spyOn(api, 'setAutostart').mockResolvedValue(undefined)
  vi.spyOn(api, 'exportDataJson').mockResolvedValue('{}')
  return vi.spyOn(api, 'setSetting').mockResolvedValue(undefined)
}

const btn = (t: string) =>
  [...document.querySelectorAll('button')].find((b) => b.textContent?.includes(t))

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
    for (const k of ['session_windows', 'daily_new_words', 'session_word_count', 'tts_provider', 'autostart_enabled']) {
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
