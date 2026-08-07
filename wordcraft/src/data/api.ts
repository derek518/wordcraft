import { invoke } from '@tauri-apps/api/core'
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

/** limit 省略时由后端读取 settings.session_word_count（决议 S13 定为 20） */
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
export const getMasteryDistribution = () =>
  call<MasteryDistribution>('get_mastery_distribution')

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

export const getSetting = (key: string) => call<string | null>('get_setting', { key })
export const setSetting = (key: string, value: string) =>
  call<void>('set_setting', { key, value })

// ── 平台 ──────────────────────────

export const playWordAudio = (word: string) => call<void>('play_word_audio', { word })
