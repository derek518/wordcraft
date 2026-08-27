/**
 * 纯前端调试用的假数据。**仅在 `VITE_MOCK=1` 下启用。**
 *
 * ```bash
 * VITE_MOCK=1 npm run dev
 * ```
 *
 * ## 为什么在这一层，而不在组件里
 *
 * 需求本身是合理的：不启 Tauri 后端时 `npm run dev` 什么都看不到，改 UI 无从验证。
 * 但先前的做法是在组件的 `catch` 里降级——那是审计 D6 的失效模式：
 * **后端一坏就自动展示假数据，真正的故障被完整盖住**。迁移 010 崩溃那次，
 * 若不是应用整个起不来，看到的会是一屏漂亮的假卡。
 *
 * 这里的区别是决定性的：
 *
 * - 假数据是**前置替换**，由环境变量在启动时选定，不是失败后的兜底
 * - 组件完全不知道它存在，`catch` 里干干净净，后端失败照样显示错误态
 * - 没有 fixture 的 command 直接抛错，而不是返回空——少一条会立刻暴露
 * - 生产构建里 `VITE_MOCK` 不为 1，整个模块被 tree-shake 掉
 */

import type { OverallStats } from '../core/types'
import type {
  BossWord,
  CollectionEntry,
  DefeatOutcome,
  DrawResult,
  SeasonState,
} from './api'

export const MOCK_ENABLED = import.meta.env.VITE_MOCK === '1'

// ── 魔王 ──────────────────────────────────

const BOSSES: BossWord[] = [
  {
    word_id: 1,
    word: 'abandon',
    phonetic: '/əˈbændən/',
    pos: 'v',
    meaning: '放弃，遗弃',
    example_1: 'They had to abandon their car in the snow.',
    lapses: 5,
    hp: 3,
    already_defeated: false,
  },
  {
    word_id: 2,
    word: 'ambiguous',
    phonetic: '/æmˈbɪɡjuəs/',
    pos: 'adj',
    meaning: '模棱两可的，含糊的',
    example_1: 'His answer was deliberately ambiguous.',
    lapses: 3,
    hp: 3,
    already_defeated: false,
  },
  {
    word_id: 3,
    word: 'consequence',
    phonetic: '/ˈkɒnsɪkwəns/',
    pos: 'n',
    meaning: '结果，后果',
    example_1: 'He was aware of the consequences of his decision.',
    lapses: 2,
    hp: 3,
    already_defeated: true,
  },
]

/** 干扰项按词区分：全部返回同一组会让「换一批选项」看起来没生效 */
const DISTRACTORS: Record<number, string[]> = {
  1: ['接受', '抛弃', '寻找', '维持', '归还', '占据'],
  2: ['清晰的', '固执的', '短暂的', '慷慨的', '陡峭的', '沉默的'],
  3: ['原因', '过程', '条件', '意图', '范围', '来源'],
}

// ── 卡牌 ──────────────────────────────────

const CARD_NAMES: [number, string, string, number, string][] = [
  [1, '翠叶碎片', 'shard', 1, 'common/grass_leaf_shard'],
  [2, '芽苗精', 'creature', 1, 'common/grass_sprout'],
  [6, '泡泡鱼', 'creature', 1, 'common/water_bubble_fish'],
  [9, '火炭碎片', 'shard', 1, 'common/fire_ember_shard'],
  [12, '火把', 'item', 1, 'common/fire_torch'],
  [17, '冰晶碎片', 'shard', 1, 'common/ice_ice_shard'],
  [24, '矿镐', 'item', 1, 'common/rock_pickaxe'],
  [25, '荆棘守卫', 'guardian', 2, 'rare/grass_thorn_guard'],
  [29, '烈焰骑士', 'guardian', 2, 'rare/fire_flame_knight'],
  [34, '永冬之镜', 'artifact', 2, 'rare/ice_eternal_mirror'],
  [39, '炎凤', 'guardian', 3, 'legend/fire_guardian'],
  [42, '岩龟', 'guardian', 3, 'legend/rock_guardian'],
]

// 混合已收集与未收集：图鉴的空槽与已得卡是两种排版，都要能看到
const CARDS: CollectionEntry[] = CARD_NAMES.map(([id, name, type, rarity, path], i) => ({
  card: {
    id,
    name,
    card_type: type,
    rarity,
    image_path: `/assets/cards/${path}.png`,
    trivia: `${name}的图鉴说明。`,
    source: '原创生成 · AI 辅助 · CC0',
  },
  count: i % 3 === 2 ? 0 : (i % 2) + 1,
  is_new: i === 1,
  first_at: i % 3 === 2 ? null : '2026-08-05T10:00:00Z',
}))

// ── 总览与赛季 ──────────────────────────────

const STATS: OverallStats = {
  total_words: 3657,
  untouched: 3347,
  total_reviews: 1206,
  total_xp: 1840,
  level: 7,
  current_streak: 5,
  best_streak: 12,
  vocab_estimate: 1382,
  draw_tickets: 15,
  makeup_cards: 1,
}

const SEASON: SeasonState = {
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
}

const ZONES = [
  { key: 'newbie', name: '新手村', total: 300, mastered: 42, unlocked: true },
  { key: 'grass', name: '清风平原', total: 600, mastered: 18, unlocked: true },
  { key: 'water', name: '蓝水湖泊', total: 600, mastered: 0, unlocked: false },
  { key: 'fire', name: '赤焰山脉', total: 600, mastered: 0, unlocked: false },
  { key: 'thunder', name: '雷霆峡谷', total: 600, mastered: 0, unlocked: false },
  { key: 'ice', name: '永冬之巅', total: 500, mastered: 0, unlocked: false },
  { key: 'rock', name: '磐石秘境', total: 457, mastered: 0, unlocked: false },
]

// ── 路由 ──────────────────────────────────

let ticketsLeft = STATS.draw_tickets

const HANDLERS: Record<string, (args?: Record<string, unknown>) => unknown> = {
  get_overall_stats: () => STATS,
  get_season: () => SEASON,
  get_collection: () => CARDS,
  get_boss_words: (args) => BOSSES.slice(0, (args?.limit as number) ?? BOSSES.length),

  get_distractor_pool: (args) => {
    const wordId = args?.wordId as number
    const limit = (args?.limit as number) ?? 3
    const pool = DISTRACTORS[wordId] ?? []
    // 每次取不同的一段，让「换一批选项」肉眼可见
    const start = Math.floor(Math.random() * Math.max(1, pool.length - limit))
    return pool.slice(start, start + limit)
  },

  defeat_boss: (args): DefeatOutcome => {
    const boss = BOSSES.find((b) => b.word_id === args?.wordId)
    return {
      word: boss?.word ?? 'unknown',
      dropped_block: !(boss?.already_defeated ?? false),
      new_question_level: 3,
    }
  },

  draw_card: (): DrawResult => {
    if (ticketsLeft <= 0) throw new Error('抽卡券不足')
    ticketsLeft -= 1
    const entry = CARDS[Math.floor(Math.random() * CARDS.length)]
    return {
      card: entry.card,
      is_first: entry.count === 0,
      count: entry.count + 1,
      tickets_left: ticketsLeft,
    }
  },

  // 设置：摸底标记为已完成，否则一进来就被引导到摸底页
  get_setting: (args) => {
    const key = args?.key as string
    return ({
      placement_stage: '2',
      onboarding_done: 'true',
      sound_enabled: 'true',
      tts_provider: 'edge',
      season_milestone_seen: '0',
      session_windows: '09:00-11:00,13:00-15:00,19:00-21:00',
      daily_new_words: '18',
      study_level: 'senior',
      study_days: '1,2,3,4,5,6,7',
    } as Record<string, string>)[key] ?? null
  },

  get_today_sessions: () => [],
  get_today_stats: () => ({ date: '2026-08-09', reviewed: 0, correct: 0, new_words: 0, xp: 0 }),
  get_zone_progress: () => ZONES,
  get_mastery_distribution: () => ({
    untouched: 3347, learning: 96, reinforcing: 62, review: 138, mastered: 14,
  }),
  get_heatmap: (args) => {
    const days = (args?.days as number) ?? 84
    // 日期由 fixture 起点递推，不取系统时间——热力图的格子位置要可复现
    const base = Date.parse('2026-08-09T00:00:00Z')
    return Array.from({ length: days }, (_, i) => ({
      date: new Date(base - (days - 1 - i) * 86400000).toISOString().slice(0, 10),
      count: [0, 0, 3, 12, 25, 8, 40][i % 7],
    }))
  },

  get_homestead: () => ({
    grid: [],
    inventory: [
      { block_type: 'normal', owned: 120, available: 120 },
      { block_type: 'rare', owned: 3, available: 3 },
      { block_type: 'limited', owned: 2, available: 2 },
    ],
    grid_size: 20,
  }),
  grant_pending_blocks: () => ({ granted: [], total_available: 125 }),
  get_residents: () => ({
    slots: 0, max_slots: 6, completed: [], residents: [], candidates: [],
    digest: { due_count: 16, available_blocks: 125, streak: 5, words_to_milestone: 90 },
  }),

  // 假数据模式下没有真词库，「一个都没导入」是事实而非谎报
  import_words: () => ({ inserted: 0, updated: 0, rejected: [] }),
  get_study_levels: () => [
    { value: 'junior', label: '初中', words: 1581 },
    { value: 'senior', label: '高中', words: 2076 },
    { value: 'cet4', label: '四级', words: 1621 },
    { value: 'all', label: '全部', words: 5278 },
  ],

  reset_learning_data_cmd: () => ({
    cleared: [['review_logs', 409], ['word_states', 1603], ['sessions', 25]],
    total_rows: 2037,
  }),

  get_ability_overview: () => ({
    vocabulary: 2504, vocabulary_low: 2100, vocabulary_high: 3010,
    frontier_from: 667, frontier_to: 4777,
    known: 666, frontier: 3120, too_hard: 1474,
    frontier_untouched: 2988, observations: 42,
  }),

  get_placement_question: () => null,
  finalize_placement: () => ({
    vocabulary: 1200, vocabulary_low: 900, vocabulary_high: 1600,
    frontier_from: 400, frontier_to: 3000,
    known: 350, frontier: 2100, too_hard: 2800,
    frontier_untouched: 1980, observations: 20,
  }),

  search_words: () => [],
  play_word_audio: () => null,
  postpone_session: () => ({ remaining: 2 }),
  get_pace: (args) => {
    const budget = (args?.dailyBudget as number) ?? 18
    const perSession = Math.ceil(budget / 3)
    return {
      new_per_session: perSession,
      session_words: Math.min(40, Math.max(12, perSession * 3)),
      weekly_new: budget * ((args?.studyDays as number) ?? 7),
    }
  },
  set_setting: () => null,
  set_autostart: () => null,
  export_data_json: () => '{"ok":true}',
  peek_popup_session: () => 'morning',
  accept_popup: () => null,
  snooze_popup: () => null,
  mark_cards_seen: () => null,
  redeem_points: () => null,
}

/**
 * 假数据入口。没有 fixture 的 command 抛错而不是返回空——
 * 静默返回 undefined 会让调用方拿到一个「成功但没有数据」的响应，
 * 那比直接失败难查得多。
 */
export function mockInvoke<T>(command: string, args?: Record<string, unknown>): T {
  const handler = HANDLERS[command]
  if (!handler) {
    throw new Error(
      `VITE_MOCK 模式下没有 \`${command}\` 的假数据。` +
        `到 src/data/devMock.ts 补一个 handler，或改用完整应用 npm run tauri dev`,
    )
  }
  return handler(args) as T
}
