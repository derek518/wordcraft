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

export const getDistractorPool = (wordId: number, pos: string, count: number) =>
  call<string[]>('get_distractor_pool', { wordId, pos, count })

// ── 作答 ──────────────────────────

export const commitReview = (payload: ReviewCommitDto) =>
  call<void>('commit_review', { payload })

// ── 会话 ──────────────────────────

export const startSession = (sessionType: SessionType, plannedCount: number) =>
  call<Session>('start_session', { sessionType, plannedCount })

export const finishSession = (sessionId: number, xpEarned: number) =>
  call<void>('finish_session', { sessionId, xpEarned })

export const getTodaySessions = () => call<Session[]>('get_today_sessions')

export const markSessionEligible = (sessionType: SessionType) =>
  call<void>('mark_session_eligible', { sessionType })

// ── 统计 ──────────────────────────

export const getOverallStats = () => call<OverallStats>('get_overall_stats')
export const getTodayStats = () => call<DayStats>('get_today_stats')
export const getMasteryDistribution = () =>
  call<MasteryDistribution>('get_mastery_distribution')

// ── 设置 ──────────────────────────

export const getSetting = (key: string) => call<string | null>('get_setting', { key })
export const setSetting = (key: string, value: string) =>
  call<void>('set_setting', { key, value })

// ── 平台 ──────────────────────────

export const playWordAudio = (word: string) => call<void>('play_word_audio', { word })
