import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, act, cleanup, fireEvent } from '@testing-library/react'
import Homestead from './Homestead'
import * as api from '../data/api'

vi.mock('../core/sound', () => ({ playCorrect: vi.fn(), playLevelUp: vi.fn() }))

const GRID = 20

function home(grid: api.PlacedBlock[] = []): api.HomesteadState {
  return {
    grid,
    inventory: [
      { block_type: 'normal', owned: 50, available: 50 - grid.length },
      { block_type: 'rare', owned: 2, available: 2 },
      { block_type: 'limited', owned: 1, available: 1 },
    ],
    grid_size: GRID,
  }
}

const RESIDENTS: api.ResidentsState = {
  slots: 0,
  max_slots: 6,
  completed: [],
  residents: [],
  candidates: [],
  digest: { due_count: 0, available_blocks: 50, streak: 0, words_to_milestone: 90 },
}

/**
 * 后端返回的是完整快照，所以桩必须维护一份真实的网格状态。
 * 若每次都回一个空网格，组件下一格的判断就基于错误的现状——
 * 那样测的是桩的缺陷，不是组件的行为
 */
function stub(initial = home()) {
  const grid = [...initial.grid]
  vi.spyOn(api, 'grantPendingBlocks').mockResolvedValue({ granted: [], total_available: 50 })
  vi.spyOn(api, 'getHomestead').mockResolvedValue(initial)
  vi.spyOn(api, 'getBlueprints').mockResolvedValue([])
  vi.spyOn(api, 'getResidents').mockResolvedValue(RESIDENTS)

  const place = vi.spyOn(api, 'placeBlock').mockImplementation(async (x, y, t) => {
    grid.push({ x, y, block_type: t })
    return home([...grid])
  })
  const remove = vi.spyOn(api, 'removeBlock').mockImplementation(async (x, y) => {
    const i = grid.findIndex((b) => b.x === x && b.y === y)
    if (i >= 0) grid.splice(i, 1)
    return home([...grid])
  })
  return { place, remove }
}

async function settle() {
  for (let i = 0; i < 3; i++) await act(async () => { await Promise.resolve() })
}

/** 网格按行优先渲染，第 n 个格子 = (n % 20, n / 20) */
const cellAt = (x: number, y: number) =>
  [...document.querySelectorAll('button')].filter((b) =>
    b.getAttribute('title')?.match(/放置到|点击移除|蓝图/),
  )[y * GRID + x]

beforeEach(() => vi.restoreAllMocks())
afterEach(cleanup)

describe('家园建造', () => {
  it('一笔涂抹只做一种操作，回划不会擦掉刚放的方块', async () => {
    // 起点空、第二格已有方块。整笔应当是「放置」，
    // 划到已有方块的格子上要跳过，而不是把它擦掉
    const { place, remove } = stub(home([{ x: 1, y: 0, block_type: 'normal' }]))

    render(<Homestead onBack={() => {}} />)
    await settle()

    await act(async () => {
      fireEvent.pointerDown(cellAt(0, 0)) // 空格 → 整笔定为「放置」
    })
    await settle()
    await act(async () => {
      fireEvent.pointerEnter(cellAt(1, 0)) // 这格有方块
    })
    await settle()

    // 先前实现按「进入格自己有没有方块」决定操作，于是这一下会擦掉它。
    // 回划到刚填好的格子同理——那正是注释声称要防的「反复放了又拆」
    expect(remove).not.toHaveBeenCalled()
    expect(place).toHaveBeenCalledTimes(1)
  })

  it('从有方块的格子起笔则整笔都是擦除', async () => {
    const { place, remove } = stub(
      home([{ x: 0, y: 0, block_type: 'normal' }, { x: 1, y: 0, block_type: 'normal' }]),
    )

    render(<Homestead onBack={() => {}} />)
    await settle()

    await act(async () => {
      fireEvent.pointerDown(cellAt(0, 0))
    })
    await settle()
    await act(async () => {
      fireEvent.pointerEnter(cellAt(1, 0))
    })
    await settle()

    expect(remove).toHaveBeenCalledTimes(2)
    expect(place).not.toHaveBeenCalled()
  })

  it('松开指针后移动不再继续涂抹', async () => {
    const { place } = stub()

    render(<Homestead onBack={() => {}} />)
    await settle()

    await act(async () => {
      fireEvent.pointerDown(cellAt(0, 0))
    })
    await settle()
    await act(async () => {
      fireEvent.pointerUp(window) // 抬手，即便指针已离开网格也要停
    })
    await act(async () => {
      fireEvent.pointerEnter(cellAt(1, 0))
    })
    await settle()

    expect(place).toHaveBeenCalledTimes(1)
  })

  it('放置失败显示错误，不静默吞掉', async () => {
    stub()
    vi.spyOn(api, 'placeBlock').mockRejectedValue(new Error('normal 方块不足'))

    render(<Homestead onBack={() => {}} />)
    await settle()
    await act(async () => {
      fireEvent.pointerDown(cellAt(0, 0))
    })
    await settle()

    expect(screen.getByText(/normal 方块不足/)).toBeTruthy()
  })

  it('尚未建成任何蓝图时不显示入住位', async () => {
    stub()
    render(<Homestead onBack={() => {}} />)
    await settle()

    // 位置由建成的蓝图解锁；一个都没建成时不该出现空槽勾人去点
    expect(document.body.textContent).toContain('建成一张蓝图就能请一只生物住进来')
  })
})
