import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'

/**
 * `call()` 的失败语义。
 *
 * 这是审计 D6 的落点：曾经组件在 `catch` 里降级到本地假数据，后端全挂时
 * 界面看起来一切正常。迁移 010 崩溃那次，若不是应用整个起不来，
 * 看到的会是一屏漂亮的假卡。
 *
 * 三个 mock 文件后来被收编到 `VITE_MOCK` 后面，但**收编本身也可能出错**——
 * 若哪天有人把开关判定写反，或在 catch 里补一条兜底，这里会立刻发现。
 */

const invoke = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }))

beforeEach(() => {
  vi.resetModules()
  invoke.mockReset()
})

afterEach(() => {
  vi.unstubAllEnvs()
})

/** 每个用例重新加载模块，好让 MOCK_ENABLED 按当前环境变量重新求值 */
async function loadApi() {
  return import('./api')
}

describe('后端调用边界', () => {
  it('后端失败时抛错，不返回任何兜底数据', async () => {
    vi.stubEnv('VITE_MOCK', '')
    invoke.mockRejectedValue('数据库锁被占用')
    const api = await loadApi()

    await expect(api.getOverallStats()).rejects.toThrow(/数据库锁被占用/)
  })

  it('错误信息带上 command 名，便于定位', async () => {
    vi.stubEnv('VITE_MOCK', '')
    invoke.mockRejectedValue('boom')
    const api = await loadApi()

    // 「失败了」没有信息量，「get_season 失败」才能直接查
    await expect(api.getSeason()).rejects.toThrow(/get_season/)
  })

  it('正常时原样返回后端结果', async () => {
    vi.stubEnv('VITE_MOCK', '')
    invoke.mockResolvedValue({ track_points: 320 })
    const api = await loadApi()

    expect(await api.getSeason()).toEqual({ track_points: 320 })
  })

  it('VITE_MOCK=1 时走假数据，且完全不碰后端', async () => {
    vi.stubEnv('VITE_MOCK', '1')
    const api = await loadApi()

    const season = await api.getSeason()
    expect(season.sessions_total).toBe(21)
    // 关键：假数据是**前置替换**而非失败兜底，所以 invoke 一次都不该被调用
    expect(invoke).not.toHaveBeenCalled()
  })

  it('假数据模式下缺 fixture 的 command 抛错，不返回空', async () => {
    vi.stubEnv('VITE_MOCK', '1')
    const api = await loadApi()

    // 静默返回 undefined 会让调用方拿到「成功但没数据」的响应，
    // 比直接失败难查得多
    await expect(api.importWords([])).rejects.toThrow(/没有.*假数据|import_words/)
  })
})
