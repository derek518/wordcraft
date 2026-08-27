/**
 * 前后端共享的数据契约。字段名与 Rust 侧序列化结果一一对应。
 * 契约来源：docs/plans/contracts-v1.md §3
 */

/** FSRS 评级。与 ts-fsrs 的 Rating 枚举同值。 */
export type Rating = 1 | 2 | 3 | 4

/** ts-fsrs State：0=New 1=Learning 2=Review 3=Relearning */
export type FsrsState = 0 | 1 | 2 | 3

/** 题型阶梯，contracts §6 */
export type QuestionType = 1 | 2 | 3 | 4 | 5

/** 业务状态机，contracts §4。与 FSRS 自身的 state 语义不同（ADR-6）。 */
export type AppState = 'new' | 'learning' | 'reinforcing' | 'review' | 'mastered'

export type SessionType = 'morning' | 'noon' | 'evening' | 'free'

/** 排队来源，用于诊断与统计 */
export type QueueSource = 'reinforcing' | 'due_review' | 'new'

/** `get_session_queue` 的返回项 */
export interface QueueItem {
  word_id: number
  word: string
  phonetic: string
  pos: string
  meaning: string
  /** 第二词性。多数词没有同样常用的第二用法，null 就是没有 */
  pos_2: string | null
  meaning_2: string | null
  example_1: string
  example_2: string
  /** Lv.5 拼写题准入判据（决议 S10：仅 1–2 段核心词开放） */
  frequency_band: number
  difficulty: number
  stability: number
  due_at: string | null
  fsrs_state: FsrsState
  app_state: AppState
  reps: number
  lapses: number
  question_level: QuestionType
  reinforce_streak: number
  /** 上次作答时刻。新词为 null。还原 FSRS Card 时必须带上，否则间隔会偏短 */
  last_review_at: string | null
  source: QueueSource
}

/** `commit_review` 的载荷，contracts §3.2 */
export interface ReviewCommitDto {
  wordId: number
  sessionId: number | null
  questionType: QuestionType
  isCorrect: boolean
  reactionMs: number
  rating: Rating
  before: {
    difficulty: number
    stability: number
  }
  after: {
    difficulty: number
    stability: number
    dueAt: string
    fsrsState: FsrsState
    reps: number
    lapses: number
  }
  appState: AppState
  questionLevel: QuestionType
  reinforceStreak: number
}

export interface Session {
  id: number
  date: string
  session_type: SessionType
  planned_count: number
  completed_count: number
  is_completed: boolean
  xp_earned: number
  postpone_count: number
}

export interface OverallStats {
  total_words: number
  untouched: number
  total_reviews: number
  total_xp: number
  level: number
  current_streak: number
  best_streak: number
  vocab_estimate: number
  draw_tickets: number
  makeup_cards: number
}

export interface MasteryDistribution {
  untouched: number
  learning: number
  reinforcing: number
  review: number
  mastered: number
  total: number
}

export interface DayStats {
  total: number
  correct: number
  again: number
  hard: number
  good: number
  easy: number
}
