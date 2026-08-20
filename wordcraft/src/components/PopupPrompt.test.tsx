import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, act, cleanup } from '@testing-library/react'
import PopupPrompt from './PopupPrompt'
import * as api from '../data/api'

async function settle() {
  for (let i = 0; i < 3; i++) await act(async () => { await Promise.resolve() })
}

const btn = (t: string) =>
  [...document.querySelectorAll('button')].find((b) => b.textContent?.includes(t))

beforeEach(() => vi.restoreAllMocks())
afterEach(cleanup)

describe('时段提示窗', () => {
  it('展示当前时段名称', async () => {
    vi.spyOn(api, 'peekPopupSession').mockResolvedValue('evening')
    render(<PopupPrompt />)
    await settle()
    expect(document.body.textContent).toContain('星夜之门')
  })

  it('开始冒险通知主窗口，稍后走延后', async () => {
    vi.spyOn(api, 'peekPopupSession').mockResolvedValue('morning')
    const accept = vi.spyOn(api, 'acceptPopup').mockResolvedValue(undefined)
    const snooze = vi.spyOn(api, 'snoozePopup').mockResolvedValue(undefined)
    render(<PopupPrompt />)
    await settle()

    await act(async () => {
      btn('开始冒险')!.click()
    })
    await settle()
    expect(accept).toHaveBeenCalledTimes(1)

    cleanup()
    vi.spyOn(api, 'peekPopupSession').mockResolvedValue('morning')
    render(<PopupPrompt />)
    await settle()
    await act(async () => {
      btn('稍后')!.click()
    })
    await settle()
    expect(snooze).toHaveBeenCalledTimes(1)
  })

  it('延后失败留在提示窗并显示原因', async () => {
    vi.spyOn(api, 'peekPopupSession').mockResolvedValue('noon')
    vi.spyOn(api, 'snoozePopup').mockRejectedValue(new Error('本时段已延后 3 次，不能再延后'))
    render(<PopupPrompt />)
    await settle()

    await act(async () => {
      btn('稍后')!.click()
    })
    await settle()

    expect(document.body.textContent).toContain('已延后 3 次')
    expect(btn('稍后')).toBeTruthy()
  })
})
