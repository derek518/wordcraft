-- WordCraft 初始 schema
-- 契约来源：docs/plans/contracts-v1.md §2
--
-- 约定（ADR-5）：
--   * 时间戳列存 UTC ISO8601 'YYYY-MM-DDTHH:MM:SSZ'
--   * date 类列存本地日期 'YYYY-MM-DD'（streak 与 session 归属按本地日计算）
--
-- 本文件已发布，禁止修改。schema 变更请新增 002_*.sql。

-- ─────────────────────────────────────────────
-- 词库
-- ─────────────────────────────────────────────
CREATE TABLE words (
  id              INTEGER PRIMARY KEY,
  word            TEXT    NOT NULL UNIQUE,
  phonetic        TEXT    NOT NULL DEFAULT '',
  pos             TEXT    NOT NULL,
  meaning         TEXT    NOT NULL,
  example_1       TEXT    NOT NULL,
  example_2       TEXT    NOT NULL DEFAULT '',
  level           TEXT    NOT NULL,
  frequency_band  INTEGER NOT NULL,
  zone            TEXT    NOT NULL,
  source_edition  TEXT    NOT NULL DEFAULT '',
  created_at      TEXT    NOT NULL,

  CHECK (level IN ('junior', 'senior', 'art')),
  CHECK (frequency_band BETWEEN 1 AND 5),
  CHECK (zone IN ('newbie', 'grass', 'water', 'fire', 'thunder', 'ice', 'rock'))
);
CREATE INDEX idx_words_zone_band ON words(zone, frequency_band);
CREATE INDEX idx_words_pos       ON words(pos);

-- ─────────────────────────────────────────────
-- 每日时段会话
-- ─────────────────────────────────────────────
CREATE TABLE sessions (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  date            TEXT    NOT NULL,
  session_type    TEXT    NOT NULL,
  planned_count   INTEGER NOT NULL,
  completed_count INTEGER NOT NULL DEFAULT 0,
  is_completed    INTEGER NOT NULL DEFAULT 0,
  xp_earned       INTEGER NOT NULL DEFAULT 0,
  postpone_count  INTEGER NOT NULL DEFAULT 0,
  merged_from     TEXT,
  started_at      TEXT,
  finished_at     TEXT,

  UNIQUE (date, session_type),
  CHECK (session_type IN ('morning', 'noon', 'evening', 'free')),
  CHECK (is_completed IN (0, 1)),
  -- spec F1：每时段最多延后 3 次
  CHECK (postpone_count BETWEEN 0 AND 3)
);
CREATE INDEX idx_sessions_date ON sessions(date);

-- ─────────────────────────────────────────────
-- FSRS 状态（ADR-6：算法状态与业务状态分列）
-- ─────────────────────────────────────────────
CREATE TABLE word_states (
  word_id           INTEGER PRIMARY KEY REFERENCES words(id) ON DELETE CASCADE,
  difficulty        REAL    NOT NULL DEFAULT 0,
  stability         REAL    NOT NULL DEFAULT 0,
  due_at            TEXT    NOT NULL,
  fsrs_state        INTEGER NOT NULL DEFAULT 0,
  app_state         TEXT    NOT NULL DEFAULT 'new',
  reps              INTEGER NOT NULL DEFAULT 0,
  lapses            INTEGER NOT NULL DEFAULT 0,
  question_level    INTEGER NOT NULL DEFAULT 1,
  reinforce_streak  INTEGER NOT NULL DEFAULT 0,
  last_review_at    TEXT,
  mastered_at       TEXT,

  -- ts-fsrs State: 0=New 1=Learning 2=Review 3=Relearning
  CHECK (fsrs_state BETWEEN 0 AND 3),
  -- 产品状态机，contracts §4
  CHECK (app_state IN ('new', 'learning', 'reinforcing', 'review', 'mastered')),
  CHECK (question_level BETWEEN 1 AND 5),
  CHECK (reinforce_streak >= 0),
  CHECK (stability >= 0),
  CHECK (difficulty >= 0)
);
CREATE INDEX idx_states_due       ON word_states(due_at);
CREATE INDEX idx_states_app_state ON word_states(app_state);

-- ─────────────────────────────────────────────
-- 作答日志
-- spec §6：记录完整信号，保证算法可回溯调参 —— 故 before/after 都存
-- ─────────────────────────────────────────────
CREATE TABLE review_logs (
  id                INTEGER PRIMARY KEY AUTOINCREMENT,
  word_id           INTEGER NOT NULL REFERENCES words(id),
  session_id        INTEGER REFERENCES sessions(id),
  question_type     INTEGER NOT NULL,
  is_correct        INTEGER NOT NULL,
  reaction_ms       INTEGER NOT NULL,
  rating            INTEGER NOT NULL,
  difficulty_before REAL    NOT NULL,
  stability_before  REAL    NOT NULL,
  difficulty_after  REAL    NOT NULL,
  stability_after   REAL    NOT NULL,
  reviewed_at       TEXT    NOT NULL,

  CHECK (question_type BETWEEN 1 AND 5),
  CHECK (is_correct IN (0, 1)),
  CHECK (reaction_ms >= 0),
  -- FSRS Rating: 1=Again 2=Hard 3=Good 4=Easy
  CHECK (rating BETWEEN 1 AND 4)
);
CREATE INDEX idx_logs_word ON review_logs(word_id, reviewed_at);
CREATE INDEX idx_logs_date ON review_logs(reviewed_at);

-- ─────────────────────────────────────────────
-- 玩家总状态（单行表）
-- ─────────────────────────────────────────────
CREATE TABLE player_stats (
  id                INTEGER PRIMARY KEY CHECK (id = 1),
  total_xp          INTEGER NOT NULL DEFAULT 0,
  level             INTEGER NOT NULL DEFAULT 1,
  current_streak    INTEGER NOT NULL DEFAULT 0,
  best_streak       INTEGER NOT NULL DEFAULT 0,
  last_streak_date  TEXT,
  vocab_estimate    INTEGER NOT NULL DEFAULT 0,
  makeup_cards      INTEGER NOT NULL DEFAULT 0,
  pause_used_month  INTEGER NOT NULL DEFAULT 0,
  draw_tickets      INTEGER NOT NULL DEFAULT 0,
  last_grant_month  TEXT,

  CHECK (total_xp >= 0),
  CHECK (level BETWEEN 1 AND 100),
  CHECK (current_streak >= 0),
  CHECK (makeup_cards >= 0),
  CHECK (draw_tickets >= 0)
);

-- ─────────────────────────────────────────────
-- 每日状态：streak 判定的事实依据（contracts §7.1）
-- eligible_count 区分「用户拒绝」与「从未弹出」——决议 S6
-- ─────────────────────────────────────────────
CREATE TABLE daily_records (
  date            TEXT    PRIMARY KEY,
  is_paused       INTEGER NOT NULL DEFAULT 0,
  eligible_count  INTEGER NOT NULL DEFAULT 0,
  completed_count INTEGER NOT NULL DEFAULT 0,
  streak_outcome  TEXT    NOT NULL DEFAULT 'pending',

  CHECK (is_paused IN (0, 1)),
  CHECK (eligible_count >= 0),
  CHECK (completed_count >= 0),
  CHECK (streak_outcome IN
    ('pending', 'increment', 'perfect', 'frozen', 'broken', 'makeup_used'))
);

-- ─────────────────────────────────────────────
-- 抽卡（决议 S9，提前进 MVP）
-- ─────────────────────────────────────────────
CREATE TABLE cards (
  id          INTEGER PRIMARY KEY,
  name        TEXT    NOT NULL,
  card_type   TEXT    NOT NULL,
  rarity      INTEGER NOT NULL,
  image_path  TEXT    NOT NULL,
  trivia      TEXT    NOT NULL DEFAULT '',
  -- spec F12 验收项：素材来源与许可证必须可追溯
  source      TEXT    NOT NULL,

  CHECK (card_type IN ('painting', 'creature')),
  CHECK (rarity BETWEEN 1 AND 3)
);

CREATE TABLE card_collection (
  card_id   INTEGER PRIMARY KEY REFERENCES cards(id),
  count     INTEGER NOT NULL DEFAULT 0,
  first_at  TEXT    NOT NULL,
  is_new    INTEGER NOT NULL DEFAULT 1,

  CHECK (count >= 0),
  CHECK (is_new IN (0, 1))
);

-- ─────────────────────────────────────────────
-- 设置
-- ─────────────────────────────────────────────
CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

-- ─────────────────────────────────────────────
-- 初始数据
-- ─────────────────────────────────────────────
INSERT INTO player_stats (id) VALUES (1);

-- 键契约见 contracts §2.1
INSERT INTO settings (key, value) VALUES
  ('schema_initialized', 'true'),
  ('onboarding_done',    'false'),
  ('placement_stage',    '0'),
  ('session_windows',    '09:00-11:00,13:00-15:00,19:00-21:00'),
  ('daily_new_words',    '6'),
  ('sound_enabled',      'true'),
  ('autostart_enabled',  'true'),
  ('tts_provider',       'edge'),
  ('daily_pause_date',   '');
