import { invoke } from '@tauri-apps/api/core'
import { MOCK_ENABLED, mockInvoke } from './devMock'
import type {
  DayStats,
  MasteryDistribution,
  OverallStats,
  QueueItem,
  ReviewCommitDto,
  Session,
  SessionType,
} from '../core/types'

/**
 * 后端 command 的唯一入口。
 *
 * 集中在此有两个理由：其一，组件不再直接依赖 Tauri，测试时替换这一层即可；
 * 其二——也是更重要的——**这里不做任何 fallback**。
 *
 * 审计 D6 的教训：原先 `WordTrainer` 在 catch 中静默切换到本地假数据，
 * 后端全挂时界面看起来一切正常。错误必须向上抛出，由组件呈现错误态。
 */

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  // 前置替换，不是失败兜底。两者的区别正是审计 D6 的要害：
  // 兜底会把真实故障伪装成正常界面，替换则由启动时的环境变量显式选定，
  // 且在 mock 模式下后端根本不参与
  if (MOCK_ENABLED) {
    return mockInvoke<T>(command, args)
  }

  try {
    return await invoke<T>(command, args)
  } catch (error) {
    // Tauri 把 Rust 的 Err(String) 原样传回，直接用作用户可见信息
    const detail = typeof error === 'string' ? error : JSON.stringify(error)
    throw new Error(`${command} 失败：${detail}`)
  }
}

// ── 词库与排队 ──────────────────────────

export interface WordImport {
  word: string
  phonetic: string
  pos: string
  meaning: string
  example_1: string
  example_2: string
  level: string
  frequency_band: number
  /**
   * 全局词频排名，能力模型的难度轴。
   *
   * `null` 表示两个语料库都未收录（18 个连字符复合词）。不插补——
   * 编一个排名会让能力模型把凭空捏造的难度当成证据。
   */
  frequency_rank: number | null
  zone: string
  source_edition: string
}

export interface ImportOutcome {
  inserted: number
  updated: number
  rejected: { word: string; reason: string }[]
}

export const importWords = (payload: WordImport[]) =>
  call<ImportOutcome>('import_words', { payload })

/** limit 省略时由后端按每日新词预算推算单场题数（见 src-tauri/src/plan.rs） */
export const getSessionQueue = (sessionType: SessionType, limit?: number) =>
  call<QueueItem[]>('get_session_queue', { sessionType, limit: limit ?? null })

/**
 * 干扰项候选。返回内容随题型翻转：Lv.1 是中文释义（看英文选中文），
 * Lv.2 以上是英文单词（看中文/听音/看例句选英文）。
 * 词性与频段等挑选条件由后端自查，此处只需给出题型等级。
 */
export const getDistractorPool = (wordId: number, questionLevel: number, count: number) =>
  call<string[]>('get_distractor_pool', { wordId, questionLevel, count })

// ── 作答 ──────────────────────────

export const commitReview = (payload: ReviewCommitDto) =>
  call<void>('commit_review', { payload })

// ── 会话 ──────────────────────────

export const startSession = (sessionType: SessionType, plannedCount: number) =>
  call<Session>('start_session', { sessionType, plannedCount })

export interface SessionResult {
  completed_count: number
  xp_earned: number
  total_xp: number
  level: number
}

export const finishSession = (sessionId: number, xpEarned: number) =>
  call<SessionResult>('finish_session', { sessionId, xpEarned })

export const getTodaySessions = () => call<Session[]>('get_today_sessions')

export const postponeSession = (sessionId: number) =>
  call<{ remaining: number }>('postpone_session', { sessionId })

export const markSessionEligible = (sessionType: SessionType) =>
  call<void>('mark_session_eligible', { sessionType })

// ── 摸底分级 ──────────────────────────

export interface PlacementQuestion {
  word_id: number
  word: string
  phonetic: string
  pos: string
  meaning: string
  band: number
  answered: number
  total: number
}

export interface PlacementOutcome {
  vocab_estimate: number
  pass_rates: number[]
  graded_review: number
  graded_learning: number
  skipped_new: number
}

export interface PlacementAnswerOutcome {
  band_closed: boolean
  placement_done: boolean
}

/** 返回 null 表示摸底已结束，该调 finalizePlacement 了 */
export const getPlacementQuestion = () =>
  call<PlacementQuestion | null>('get_placement_question')

export const submitPlacementAnswer = (
  wordId: number,
  band: number,
  isCorrect: boolean,
  reactionMs: number,
) =>
  call<PlacementAnswerOutcome>('submit_placement_answer', {
    wordId,
    band,
    isCorrect,
    reactionMs,
  })

export const finalizePlacement = () => call<PlacementOutcome>('finalize_placement')

// ── 抽卡与图鉴 ──────────────────────────

export interface Card {
  id: number
  name: string
  card_type: string
  rarity: number
  image_path: string
  trivia: string
  source: string
}

export interface DrawResult {
  card: Card
  is_first: boolean
  count: number
  tickets_left: number
}

export interface CollectionEntry {
  card: Card
  count: number
  is_new: boolean
  first_at: string | null
}

/** 券不足时后端返回 Err，此处照常抛出——静默失败会让用户以为按钮坏了 */
export const drawCard = () => call<DrawResult>('draw_card')
export const getCollection = () => call<CollectionEntry[]>('get_collection')
export const markCardsSeen = (cardIds: number[]) =>
  call<void>('mark_cards_seen', { cardIds })

// ── 统计 ──────────────────────────

export const getOverallStats = () => call<OverallStats>('get_overall_stats')
export const getTodayStats = () => call<DayStats>('get_today_stats')
export interface HeatmapCell {
  date: string
  count: number
}

/** 日历热力图。后端补齐缺失日期为 0，前端按固定网格渲染不会错位 */
export const getHeatmap = (days: number) => call<HeatmapCell[]>('get_heatmap', { days })

export const searchWords = (keyword: string, limit: number) =>
  call<LibraryWord[]>('search_words', { keyword, limit })

export interface LibraryWord {
  id: number
  word: string
  phonetic: string
  pos: string
  meaning: string
  example_1: string
  example_2: string
  level: string
  frequency_band: number
  zone: string
}

export const getMasteryDistribution = () =>
  call<MasteryDistribution>('get_mastery_distribution')

// ── 家园建造 ──────────────────────────

export interface PlacedBlock {
  x: number
  y: number
  block_type: string
}

export interface BlockStock {
  block_type: string
  owned: number
  available: number
}

export interface HomesteadState {
  grid: PlacedBlock[]
  inventory: BlockStock[]
  /** 网格边长，前端不硬编码 */
  grid_size: number
}

export interface GrantOutcome {
  granted: [string, number][]
  total_available: number
}

export const getHomestead = () => call<HomesteadState>('get_homestead')

/** 放置与移除都回传完整快照——前端自行推算库存必然与后端错开 */
export const placeBlock = (x: number, y: number, blockType: string) =>
  call<HomesteadState>('place_block', { x, y, blockType })

export const removeBlock = (x: number, y: number) =>
  call<HomesteadState>('remove_block', { x, y })

/** 幂等，可随时调用 */
export const grantPendingBlocks = () => call<GrantOutcome>('grant_pending_blocks')

export interface BlueprintCell {
  x: number
  y: number
  block_type: string
}

export interface Blueprint {
  id: string
  name: string
  description: string
  /** 1..4。后一张严格包含前一张，所以也是解锁顺序 */
  stage: number
  cells: BlueprintCell[]
  required: [string, number][]
}

/** 静态内容，随版本发布；不落库因为没有用户态可存 */
export const getBlueprints = () => call<Blueprint[]>('get_blueprints')

export interface Resident {
  /** 已入住时是位置序号，候选时为 -1 */
  slot: number
  card_id: number
  name: string
  image_path: string
  rarity: number
}

/** 居民转述的真实数字，措辞在前端 */
export interface HomesteadDigest {
  due_count: number
  available_blocks: number
  streak: number
  /** 距下一个词量里程碑还差多少词；0 表示没有下一档 */
  words_to_milestone: number
}

export interface ResidentsState {
  slots: number
  max_slots: number
  completed: string[]
  residents: Resident[]
  candidates: Resident[]
  digest: HomesteadDigest
}

export const getResidents = () => call<ResidentsState>('get_residents')

export const moveInResident = (slot: number, cardId: number) =>
  call<ResidentsState>('move_in_resident', { slot, cardId })

export const moveOutResident = (slot: number) =>
  call<ResidentsState>('move_out_resident', { slot })

// ── 魔王讨伐 ──────────────────────────

export interface BossWord {
  word_id: number
  word: string
  phonetic: string
  pos: string
  meaning: string
  example_1: string
  /** 遗忘次数，即这个魔王「击败过你」多少次 */
  lapses: number
  hp: number
  already_defeated: boolean
}

export interface DefeatOutcome {
  word: string
  /** 重复讨伐同一个魔王不再掉落 */
  dropped_block: boolean
  new_question_level: number
}

export const getBossWords = (limit: number) => call<BossWord[]>('get_boss_words', { limit })
export const defeatBoss = (wordId: number) =>
  call<DefeatOutcome>('defeat_boss', { wordId })

// ── 赛季赛道 ──────────────────────────

export interface SeasonState {
  week_start: string
  week_end: string
  sessions_done: number
  sessions_total: number
  progress: number
  /** 幽灵车 = 上周同期的自己，spec 明确排除社交对比 */
  ghost_progress: number
  ghost_sessions: number
  projected_points: number
  track_points: number
  /** 计分参数，来自后端 scoring.rs——前端不写死，否则改价时必然漂移 */
  points_per_session: number
  perfect_bonus: number
}

export interface RedeemOutcome {
  track_points: number
  draw_tickets: number
  makeup_cards: number
}

export const getSeason = () => call<SeasonState>('get_season')

/** 积分不足时后端返回 Err，照常抛出 */
export const redeemPoints = (item: string) =>
  call<RedeemOutcome>('redeem_points', { item })

// ── 区域进度 ──────────────────────────

export interface ZoneProgress {
  key: string
  name: string
  total: number
  learned: number
  unlocked: boolean
  required_level: number
}

/** 词数与解锁状态一律现查——前端硬编码过一次，六个数字里五个是错的 */
export const getZoneProgress = () => call<ZoneProgress[]>('get_zone_progress')

// ── 设置 ──────────────────────────

export interface StudyLevelOption {
  value: string
  label: string
  words: number
}

export interface AbilityOverview {
  /** 估计的词汇量 */
  vocabulary: number
  /** ±1 标准误换算成的词汇量区间 */
  vocabulary_low: number
  vocabulary_high: number
  /** 学习前沿的词频排名区间 */
  frontier_from: number
  frontier_to: number
  /** 词库按能力分层的词数 */
  known: number
  frontier: number
  too_hard: number
  /** 前沿里还没学过的词数 */
  frontier_untouched: number
  /** 参与估计的观测数。为 0 表示还在用先验 */
  observations: number
}

/**
 * 能力概览：水平估到哪，重点该放在哪一段。
 *
 * 这是「学习范围」的替代品。范围不再由家长猜，而是由每天的作答算出来。
 */
export const getAbilityOverview = () => call<AbilityOverview>('get_ability_overview')

/** 可选学习范围。词数由后端现查——写死的计数在本项目已三次变成谎话 */
export const getStudyLevels = () => call<StudyLevelOption[]>('get_study_levels')

/**
 * 每日新词预算的缺省值。**唯一允许存在的副本**，与 `plan::DEFAULT` 对应。
 *
 * 只在设置面板读到值之前占位；`settings` 表由迁移 001 播种，正常路径上
 * 永远读得到，所以这个值不会真正影响学习节奏。
 */
export const DEFAULT_DAILY_NEW = 18

export interface Pace {
  /** 三时段均分时每场的新词数 */
  new_per_session: number
  /** 每场题数 */
  session_words: number
  /** 每周新词数 */
  weekly_new: number
}

/**
 * 每日预算推算出的节奏。纯投影，不读库——传入的是滑块当前值而非已保存值，
 * 这样拖动时数字即时跟着走。系数与上下限都在后端，界面不留副本。
 */
export const getPace = (dailyBudget: number, studyDays: number) =>
  call<Pace>('get_pace', { dailyBudget, studyDays })

export const getSetting = (key: string) => call<string | null>('get_setting', { key })
export const setSetting = (key: string, value: string) =>
  call<void>('set_setting', { key, value })

export interface ResetSummary {
  /** 被清空的表及行数 */
  cleared: [string, number][]
  total_rows: number
}

/**
 * 清空全部学习与游戏进度，保留词库与家长配置。
 *
 * 返回清空明细而非 void：这个操作不可逆，「点了没反应」和「清干净了」
 * 在界面上必须能区分。
 */
export const resetLearningData = () => call<ResetSummary>('reset_learning_data_cmd')

export const setAutostart = (enabled: boolean) =>
  call<void>('set_autostart', { enabled })

export const exportDataJson = () => call<string>('export_data_json')

export const peekPopupSession = () => call<string | null>('peek_popup_session')
export const acceptPopup = () => call<void>('accept_popup')
export const snoozePopup = () => call<void>('snooze_popup')

// ── 平台 ──────────────────────────

export const playWordAudio = (word: string) => call<void>('play_word_audio', { word })
