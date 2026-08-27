# WordCraft V1 契约文档

> 本文件是**契约**，不是实现。所有任务实施必须遵守此处定义的 schema / 签名 / 状态机。
> 契约变更需先改本文件，再改代码。
> 依据：`wordcraft-spec.md` (v0.1) + `wordcraft-spec-v1.0.md`

---

## 1. 架构决策记录（ADR）

| # | 决策 | 理由 |
|---|---|---|
| ADR-1 | **SQLite** 替代 JSON 文件存储 | spec §5 强制；5100 词 + 累积 review_logs 下 JSON 全量读写不可行；需要事务保证 |
| ADR-2 | **FSRS 在前端**（`ts-fsrs`），Rust 只持久化 | spec v1.0 §6.1 原意；ts-fsrs 生态成熟；Rust 侧只需按 `due_at` 查询排队，不需算法 |
| ADR-3 | **平台抽象层** 隔离 Windows API | 开发机为 macOS，目标为 Windows；trait + `#[cfg(target_os)]` 双实现，保证本地可跑除全屏检测外全部逻辑 |
| ADR-4 | **`chrono`** 处理全部日期时间，禁止手写日历 | 审计 D2：手写实现 85% 日期算错 |
| ADR-5 | 存储层时间戳一律 **UTC ISO8601**；日期归属（"今天"）按 **本地时区** 计算 | 跨时区/DST 正确性；streak 与 session 归属必须用本地日 |
| ADR-6 | FSRS 状态与业务状态**分离为两列** | `fsrs_state` 由 ts-fsrs 拥有；`app_state` 是 spec F2 的产品状态机，二者语义不同 |

---

## 2. 数据库 Schema（migration 001）

> 全部时间列为 UTC ISO8601 字符串 `YYYY-MM-DDTHH:MM:SSZ`，`date` 类列为本地日期 `YYYY-MM-DD`。

```sql
CREATE TABLE schema_migrations (
  version     INTEGER PRIMARY KEY,
  applied_at  TEXT NOT NULL
);

CREATE TABLE words (
  id              INTEGER PRIMARY KEY,
  word            TEXT NOT NULL UNIQUE,
  phonetic        TEXT NOT NULL DEFAULT '',
  pos             TEXT NOT NULL,                    -- 'n.' | 'v.' | 'adj.' | 'adv.' | ...
  meaning         TEXT NOT NULL,
  example_1       TEXT NOT NULL,
  example_2       TEXT NOT NULL DEFAULT '',
  level           TEXT NOT NULL,                    -- 'junior' | 'senior' | 'cet4' | 'art'
  frequency_band  INTEGER NOT NULL,                 -- 1..5, 1 = 最高频
  frequency_rank  INTEGER,                          -- 全局词频排名（迁移 013）；可空
  zone            TEXT NOT NULL,                    -- 'newbie'|'grass'|'water'|'fire'|'thunder'|'ice'|'rock'
  created_at      TEXT NOT NULL
);
CREATE INDEX idx_words_zone_band ON words(zone, frequency_band);
CREATE INDEX idx_words_rank ON words(frequency_rank);
CREATE INDEX idx_words_pos       ON words(pos);     -- 干扰项按同词性检索

CREATE TABLE word_states (
  word_id           INTEGER PRIMARY KEY REFERENCES words(id) ON DELETE CASCADE,
  difficulty        REAL    NOT NULL DEFAULT 0,     -- FSRS D
  stability         REAL    NOT NULL DEFAULT 0,     -- FSRS S，单位：天
  due_at            TEXT    NOT NULL,
  fsrs_state        INTEGER NOT NULL DEFAULT 0,     -- ts-fsrs State: 0=New 1=Learning 2=Review 3=Relearning
  app_state         TEXT    NOT NULL DEFAULT 'new', -- 见 §4 业务状态机
  reps              INTEGER NOT NULL DEFAULT 0,
  lapses            INTEGER NOT NULL DEFAULT 0,
  question_level    INTEGER NOT NULL DEFAULT 1,     -- 1..5 题型阶梯
  reinforce_streak  INTEGER NOT NULL DEFAULT 0,     -- 强化队列内「8 秒内答对」连续次数
  last_review_at    TEXT,
  mastered_at       TEXT
);
CREATE INDEX idx_states_due       ON word_states(due_at);
CREATE INDEX idx_states_app_state ON word_states(app_state);

CREATE TABLE review_logs (
  id                INTEGER PRIMARY KEY AUTOINCREMENT,
  word_id           INTEGER NOT NULL REFERENCES words(id),
  session_id        INTEGER REFERENCES sessions(id),
  question_type     INTEGER NOT NULL,               -- 1..5
  is_correct        INTEGER NOT NULL,               -- 0 | 1
  reaction_ms       INTEGER NOT NULL,
  rating            INTEGER NOT NULL,               -- FSRS Rating: 1=Again 2=Hard 3=Good 4=Easy
  difficulty_before REAL NOT NULL,
  stability_before  REAL NOT NULL,
  difficulty_after  REAL NOT NULL,
  stability_after   REAL NOT NULL,
  reviewed_at       TEXT NOT NULL
);
CREATE INDEX idx_logs_word ON review_logs(word_id, reviewed_at);
CREATE INDEX idx_logs_date ON review_logs(reviewed_at);

CREATE TABLE sessions (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  date            TEXT NOT NULL,                    -- 本地日期
  session_type    TEXT NOT NULL,                    -- 'morning'|'noon'|'evening'|'free'
  planned_count   INTEGER NOT NULL,
  completed_count INTEGER NOT NULL DEFAULT 0,
  is_completed    INTEGER NOT NULL DEFAULT 0,
  xp_earned       INTEGER NOT NULL DEFAULT 0,
  postpone_count  INTEGER NOT NULL DEFAULT 0,       -- spec F1：每时段最多 3 次
  merged_from     TEXT,                             -- 被合并的上一时段 session_type
  started_at      TEXT,
  finished_at     TEXT,
  UNIQUE(date, session_type)
);

CREATE TABLE player_stats (
  id                INTEGER PRIMARY KEY CHECK (id = 1),
  total_xp          INTEGER NOT NULL DEFAULT 0,
  level             INTEGER NOT NULL DEFAULT 1,
  current_streak    INTEGER NOT NULL DEFAULT 0,
  best_streak       INTEGER NOT NULL DEFAULT 0,
  last_streak_date  TEXT,
  vocab_estimate    INTEGER NOT NULL DEFAULT 0,     -- 由 ability_theta 推算（§13），每次首见作答刷新
  makeup_cards      INTEGER NOT NULL DEFAULT 0,     -- 补签卡，每月 1 日自动发 2 张（S4）
  pause_used_month  INTEGER NOT NULL DEFAULT 0,     -- 今日暂停已用次数，每月限 2
  draw_tickets      INTEGER NOT NULL DEFAULT 0,     -- 抽卡券（S9）
  last_grant_month  TEXT,                           -- 上次发放补签卡的月份 'YYYY-MM'，防重复发放

  -- 能力估计（迁移 014，见 §13）。三者可空 / 为 0 表示尚无观测，回落到先验
  ability_theta        REAL,                         -- 能力值，log2(词频排名) 尺度
  ability_information  REAL NOT NULL DEFAULT 0,      -- 累计 Fisher 信息量，决定单次观测的步长
  ability_observations INTEGER NOT NULL DEFAULT 0    -- 参与估计的观测数（仅首见作答）
);

-- 每日状态：streak 判定的事实依据，支持回溯（S1/S6/S8）
CREATE TABLE daily_records (
  date            TEXT PRIMARY KEY,                 -- 本地日期
  is_paused       INTEGER NOT NULL DEFAULT 0,       -- 今日暂停是否激活
  eligible_count  INTEGER NOT NULL DEFAULT 0,       -- 当日「实际弹出过或用户主动发起」的时段数
  completed_count INTEGER NOT NULL DEFAULT 0,
  streak_outcome  TEXT NOT NULL DEFAULT 'pending'   -- pending|increment|perfect|frozen|broken|makeup_used
);

-- 卡池定义（随包分发，非用户数据）
CREATE TABLE cards (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  card_type   TEXT NOT NULL,                        -- 'painting' | 'creature'
  rarity      INTEGER NOT NULL,                     -- 1=普通 2=稀有 3=传说
  image_path  TEXT NOT NULL,
  trivia      TEXT NOT NULL DEFAULT '',             -- 画作冷知识，spec F12
  source      TEXT NOT NULL                         -- 素材来源与许可证，spec F12 验收项
);

CREATE TABLE card_collection (
  card_id     INTEGER PRIMARY KEY REFERENCES cards(id),
  count       INTEGER NOT NULL DEFAULT 0,
  first_at    TEXT NOT NULL,
  is_new      INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
```

### 2.1 settings 键契约

| key | 默认值 | 说明 |
|---|---|---|
| `schema_initialized` | `"true"` | 初始化完成标记（替代原 `first_run`） |
| `onboarding_done` | `"false"` | 摸底测试是否完成 |
| `session_windows` | `"09:00-11:00,13:00-15:00,19:00-21:00"` | spec F1 三时段 |
| `daily_new_words` | `"18"` | **每日**新词预算（迁移 012 起；此前为每场，旧值 ×3 换算）。学习量的唯一旋钮——单场题数与每场新词配额都由它推算，见 `src-tauri/src/plan.rs`。**运行时仍受 §4.1 自适应调制**，此为上限而非定值 |
| `placement_stage` | `"0"` | 摸底进度：0=未开始 1=进行中 2=已完成（支持分两次） |
| `daily_pause_date` | `""` | 今日暂停激活的日期，空串表示未激活 |
| `sound_enabled` | `"true"` | |
| `autostart_enabled` | `"true"` | |
| `tts_provider` | `"edge"` | `edge` \| `sapi` \| `off` |
| `season_milestone_seen` | `"0"` | 已庆祝过的最高赛道里程碑（时段数）。庆祝在**跨过的那一刻放一次**，靠这个键保证不会每次打开赛道页都重放 |
| `postpone_until` | `""` | 延后到期时刻（UTC ISO8601）。空串表示当前没有延后。调度器在此之前不重复弹出同一时段 |
| `postpone_session_type` | `""` | 正在延后的时段 `morning`/`noon`/`evening` |
| `study_level` | `"all"` | **可选**的考纲约束：`junior`/`senior`/`cet4`/`all`。**不是难度选择器**——难度由 §13 的能力模型负责，考纲标签和难度基本无关。默认全库；值非法时回落到 `all` 并记 warn |
| `study_days` | `"1,2,3,4,5,6,7"` | 弹出训练的 ISO 星期（1=周一）。赛道分母跟随它缩放，取消工作日不算断签。不允许清空 |
| `library_fingerprint` | `""` | 词库内容指纹。变了就重新导入——词库扩充后老用户拿不到新词，而界面上看不出异常。**仅在零拒绝时写入**，否则失败的导入会自我标记为完成 |

> **迁移 012 · 学习量合并为单一旋钮**
>
> `session_word_count` 已删除，`daily_new_words` 语义由「每场」改为「每日」。
>
> 原因：两者之间存在物理约束——每个新词当天还会带来若干次复习，单场题数
> 不可能独立于新词量取值。两个旋钮各自可调时能配出无法满足的组合
> （「每场 40 题、每天 3 个新词」时那 37 题无处可来），队列静静地给不满，
> 而界面上看不出任何异常。
>
> 另一个原因是 `daily_new_words` 的旧语义有误导性：后端在**每个时段**的
> `build()` 里各读一次，三时段就是三倍——设 14 实际是每天 42 个。
> 迁移 012 把旧值 ×3（封顶 60）以保持用户当前的实际学习量不变。
>
> 推算规则见 `src-tauri/src/plan.rs`：预算按**剩余时段**均分（跳过早场则中午
> 和晚上各领一半）；单场题数 = 每场新词 × 3，夹在 12–40 之间。
> `get_pace(daily_budget, study_days)` 供界面展示推算结果，是纯投影，不读库。

> **预算按「学习负担」扣，不按「词数」扣**
>
> 一个一眼就答对的词，对孩子来说本来就算不上新词——它几乎不产生后续复习。
> 把它和一个完全不会的词同等扣预算，等于因为「今天遇到的词碰巧都会」而提前
> 收工。
>
> 消耗按**首答评级**加权（`plan::cost_of`）。权重来自实测（真实库 165 词 /
> 409 次作答，按首答评级分组的平均总作答次数）：
>
> ```text
> 评级        平均总作答   首次间隔    权重
> 1 Again        5.85      1.0 天      1.00
> 2 Hard         3.30      4.5 天      0.55
> 3 Good         3.11      4.2 天      0.55
> 4 Easy         1.52     10.4 天      0.25
> ```
>
> 差异来自 FSRS 的初始间隔，是算法机制而非个人特征——换个人作答，给定评级
> 之后的负担比例不变。Hard 与 Good 实测几乎没有区别（3.30 / 3.11），合成一档。
>
> **词数另有硬上限** `MAX_RAW_MULTIPLIER = 2`：全 Easy 时按 0.25 折算本可给
> 四倍，压到两倍，否则单场会长到坐不住（目标用户有 ADHD 特征）。水平真的高
> 的话 θ 会跟着涨、前沿上移，词自然就不再是 Easy——这条上限只是兜住过渡期。
>
> 于是 `daily_new_words` 限的是**当天愿意承担多少学习负担**：一个词都不会的
> 一天就是设定值本身，遇到的词基本都会的一天能给到两倍。界面展示区间而非
> 单值（`Pace` 的 `*_max` 字段），上下限都由后端算。
>
> 首见判据用 `MIN(id)` 而非 `MIN(reviewed_at)`：时间戳只到秒，同一个词在一场
> 里被重排两次可能撞上同一秒，按时间戳取会取出两行。

---

## 3. Tauri Command 签名契约

> 命名规则：`snake_case`（Rust 侧），前端 `invoke` 传参用 `camelCase`（Tauri 2 自动转换）。
> 所有命令返回 `Result<T, String>`；错误必须携带可诊断信息，**禁止吞错返回 `Ok(())`**。

### 3.1 词库 / 排队

```rust
/// 按 §4.1 自适应配额返回本次 session 的词。
/// 排队优先级：强化中 > 到期复习(due_at<=now) > 摸底抽查 > 新词(受 daily_new_words 限额)
/// limit 省略时由 plan.rs 按 daily_new_words 推算单场题数
get_session_queue(session_type: String, limit: Option<i64>) -> Result<Vec<QueueItem>, String>

/// 返回指定词的干扰项候选池（同词性、编辑距离近的词），由前端组题。
/// 干扰项候选池。返回内容随题型翻转：Lv.1 返回释义（看英文选中文），
/// Lv.2-5 返回单词（看中文/听音/看例句选英文）。
/// 词性、频段等挑选条件由后端从 word_id 自查，前端不需了解规则。
get_distractor_pool(word_id: i64, question_level: i64, count: i64) -> Result<Vec<String>, String>

/// 批量导入词库。冲突时按 word 唯一键 upsert，返回 (inserted, updated)。
import_words(payload: Vec<WordImportDto>) -> Result<ImportResult, String>

search_words(keyword: String, limit: i64) -> Result<Vec<Word>, String>
```

### 3.2 作答持久化

```rust
/// 前端用 ts-fsrs 算完新状态后调用，一次事务写入 word_states + review_logs。
/// Rust 侧不做 FSRS 计算，只做持久化与校验（ADR-2）。
commit_review(payload: ReviewCommitDto) -> Result<(), String>
```

```typescript
// 前端 → Rust 的载荷契约
interface ReviewCommitDto {
  wordId: number
  sessionId: number | null
  questionType: 1 | 2 | 3 | 4 | 5
  isCorrect: boolean
  reactionMs: number
  rating: 1 | 2 | 3 | 4              // FSRS Rating
  before: { difficulty: number; stability: number }
  after: {
    difficulty: number
    stability: number
    dueAt: string                    // UTC ISO8601
    fsrsState: 0 | 1 | 2 | 3
    reps: number
    lapses: number
  }
  appState: AppState                 // 前端按 §4 状态机计算后下发
  questionLevel: 1 | 2 | 3 | 4 | 5
  reinforceStreak: number
}
```

### 3.3 Session 生命周期

```rust
start_session(session_type: String, planned_count: i64) -> Result<i64, String>  // 返回 session_id
finish_session(session_id: i64, xp_earned: i64) -> Result<SessionResult, String>
get_today_sessions() -> Result<Vec<Session>, String>

/// 延后 15 分钟。返回剩余次数；已达 3 次返回 Err。
/// 若延后后超出时段窗口，自动并入下一时段而非判失败（决议 S12）。
postpone_session(session_id: i64) -> Result<PostponeResult, String>

/// 时段实际弹出时调用，写 daily_records.eligible_count —— streak 判定的分母（决议 S6）
mark_session_eligible(session_type: String) -> Result<(), String>

/// 今日暂停：冻结语义，streak 不增不减（决议 S8）。月配额耗尽返回 Err。
activate_daily_pause() -> Result<i64, String>   // 返回本月剩余次数

/// 日终结算：按 §7.1 判定 streak 走向，写 daily_records.streak_outcome
settle_day(date: String) -> Result<StreakOutcome, String>
```

### 3.6 摸底测试（§9）

```rust
/// 下一道摸底题：取离能力边界最近、且没问过的词（§9.1）。None = 20 题答完
get_placement_question() -> Result<Option<PlacementQuestion>, String>

/// 提交一题。喂给 θ 的观测与日常首见作答**同一种**——都是没教过就直接考
submit_placement_answer(word_id: i64, is_correct: bool, reaction_ms: i64)
    -> Result<AnswerOutcome, String>

/// 结算。返回的就是设置页那张卡的内容——摸底不再产出一套自己的数字
finalize_placement() -> Result<AbilityOverview, String>

pub struct PlacementQuestion {
    pub word_id: i64, pub word: String, pub phonetic: String,
    pub pos: String, pub meaning: String,
    pub frequency_rank: i64,        // 界面用它显示「这题有多难」
    pub answered: i64, pub total: i64,
}

pub struct AnswerOutcome { pub answered: i64, pub total: i64, pub placement_done: bool }
```

`get_placement_question` / `submit_placement_answer` 的主体分别抽成
`next_question()` / `record_answer()`，测试打在真代码上——在测试里重写一遍
更新逻辑的话，改坏生产代码它一声不吭（本项目已两次栽在这上面）。

### 3.4 统计

```rust
get_today_stats() -> Result<TodayStats, String>
get_overall_stats() -> Result<OverallStats, String>
get_mastery_distribution() -> Result<MasteryDistribution, String>  // 五段色条
get_heatmap(days: i64) -> Result<Vec<HeatmapCell>, String>
export_data_json() -> Result<String, String>                       // spec F7 导出
```

### 3.8 数据重置

```rust
/// 清空全部学习与游戏进度，恢复成「全新一台」。
///
/// 返回清空明细而非 `Ok(())`——操作不可逆，「点了没反应」和「清干净了」
/// 在界面上必须能区分。
reset_learning_data_cmd() -> Result<ResetSummary, String>

pub struct ResetSummary {
    pub cleared: Vec<(String, i64)>,   // 表名 → 清空行数，仅含非空表
    pub total_rows: i64,
}
```

**边界**（`src-tauri/src/reset.rs`）：

| | 内容 |
|---|---|
| 清空 | `review_logs` `sessions` `homestead_grid` `homestead_residents` `card_collection` `block_grants` `word_states` `placement_asked` `placement_results` `daily_records` `season_settlements`；`player_stats` 删除后按 schema 默认值重建 |
| 计数归零 | `block_inventory` 的 `owned`/`placed`——那三行是**结构种子**（三种方块类型），删掉会让 `homestead_grid` 的外键无处可指 |
| 保留 | `words` `cards`，以及全部家长配置 |
| 复位 | `onboarding_done`→`false`、`placement_stage`→`0`、`daily_pause_date`、`season_milestone_seen`→`0`、`postpone_until`、`postpone_session_type` |

**为什么连游戏进度一起清**：等级、方块、卡牌与作答记录互相引用，只清一半会留下
「魔王已讨伐但那个词又变回生词」这类自相矛盾的状态。而且早期的升级与解锁本身
就是动机设计的一部分，让孩子从 Lv.11 起步等于把这段体验删掉。

单事务执行，提交前跑 `pragma_foreign_key_check`，有悬空引用则回滚。

### 3.5 系统集成（平台抽象，ADR-3）

```rust
get_next_session_time() -> Result<SessionTime, String>   // 真实计算，禁止硬编码
/// 弹出 360×480 无焦点提示窗（右下角、alwaysOnTop）。不得对主窗口 set_focus。
trigger_popup_now() -> Result<(), String>
peek_popup_session() -> Result<Option<String>, String>
accept_popup() -> Result<(), String>                     // 关掉提示窗，主窗口前置并开始该时段
snooze_popup() -> Result<(), String>                     // 延后 15 分钟并关掉提示窗
set_autostart(enabled: bool) -> Result<(), String>       // 同步系统自启与 settings.autostart_enabled
get_user_busy_state() -> Result<BusyState, String>       // Windows: SHQueryUserNotificationState
play_word_audio(word: String) -> Result<(), String>      // 缓存未命中时先合成再播
prefetch_audio(words: Vec<String>) -> Result<i64, String>
```

```rust
// 平台抽象 trait（src-tauri/src/platform/mod.rs）
pub trait PlatformIntegration: Send + Sync {
    fn user_busy_state(&self) -> Result<BusyState, PlatformError>;
    fn set_autostart(&self, enabled: bool) -> Result<(), PlatformError>;
    fn speak_fallback(&self, text: &str) -> Result<(), PlatformError>;
}
// windows.rs → 真实实现；stub.rs → 仅 #[cfg(not(target_os="windows"))]，
// 且必须返回 BusyState::Unknown 并记 warn 日志，禁止假装成 Normal。

pub enum BusyState { Normal, FullScreenD3D, Busy, Presentation, Unknown }
```

---

## 4. 业务状态机（spec F2）

```
                  ┌──────────────────────────────────────┐
                  │                                      │ 抽查失败
   new ──首次作答──> learning ──答对──> review ──S>60d且Lv≥4通过──> mastered
                       ▲                  │                        │
                       │                  │答错                    │
                       │                  ▼                        │
                       └──────────── reinforcing <─────────────────┘
                        连续 3 次 8s 内答对
```

**转移规则（前端计算，Rust 只持久化）**

| 当前 | 事件 | 目标 | 副作用 |
|---|---|---|---|
| `new` | 答对 | `learning` | `question_level = 1`（不得直接跳到 `review`） |
| 任意 | 答错 | `reinforcing` | **优先于上一行**：`reinforce_streak = 0`；当场排入本 session 队尾；`question_level` 降 1（最低 1） |
| `reinforcing` | 答对且 `reaction_ms < 8000` | `reinforce_streak += 1`；**达 2 → `review`** | 未达 2 保持 `reinforcing`（决议 S3） |
| `reinforcing` | 答对但 `reaction_ms >= 8000` | 保持 | `reinforce_streak = 0`（重新计数） |
| `learning` | 答对 | `review` | `question_level += 1`（封顶 5） |
| `review` | `stability > 60` 且本次 `question_level >= 4` 且答对 | `mastered` | 记 `mastered_at`；转入 60–90 天低频抽查 |
| `mastered` | 抽查答错 | `reinforcing` | `mastered_at = NULL` |

### 4.1 强化队列自适应控制（决议 S3）

spec 原方案（连续 3 次离队 + 固定 40% 配额）经 180 天蒙特卡洛模拟验证**永不收敛**（池增长至 274 词）。改为三档自适应：

```
R = 强化池大小 = COUNT(*) WHERE app_state = 'reinforcing'

新词额度 daily_new_words_effective:
  R <= 15          -> 配置值（默认 6）
  15 < R <= 30     -> ceil(配置值 / 2)
  R > 30           -> 0

强化配额 reinforce_ratio:
  R <= 15          -> 0.40
  15 < R <= 30     -> 0.50
  R > 30           -> 0.60
```

**排队占比约束**：`get_session_queue` 返回结果中 `app_state='reinforcing'` 的词数 ≥ `ceil(limit * reinforce_ratio)`，强化池不足则全取。

### 4.2 强化队列的到期日覆盖（2026-08-06 实跑发现）

**`app_state = 'reinforcing'` 时，`due_at` 强制不晚于次日**，覆盖 FSRS 的建议值。

FSRS 只看记忆曲线，不知道「强化队列」这个产品概念。一个错词若在重考时被
轻松答对，FSRS 会给出长间隔（实测出现过 8 天），但此时它的 `reinforce_streak`
可能才 1——离队需要连续 2 次，第二次却要等 8 天后。强化队列因此形同虚设。

spec F2 明确要求错词「次日必现」，这是产品规则，优先于算法建议：

```
if app_state == 'reinforcing':
    due_at = min(fsrs_due_at, now + 24h)
```

> ADR-6 把 `fsrs_state` 与 `app_state` 分列已预见算法与产品的分歧，但只分了
> 状态未分到期日。此处补齐。实现位于前端 `src/core/fsrs.ts`（ADR-2）。

> 正常状态（R≤15）完全等同 spec 原体验。三档而非两档是为了阻尼——两档会在阈值附近反复横跳。
> 用户不可见此机制，不做 UI 暴露（避免「系统在惩罚我」的感受）。

---

## 5. 自动评级映射（spec F2，禁止 Anki 式自评）

```
输入: is_correct, reaction_ms, question_type
输出: FSRS Rating (1=Again 2=Hard 3=Good 4=Easy)

if !is_correct                        -> Again(1)
else if reaction_ms <  FAST[qt]       -> Easy(4)
else if reaction_ms <  SLOW[qt]       -> Good(3)
else                                  -> Hard(2)
```

**阈值按题型独立定义（决议 S5）**——绝对阈值对输入型题目不成立：`perspective` 光打字就要 3–5 秒，用 3s/8s 判定会让完全掌握该词的用户永远拿不到 Easy，甚至被判 Hard 导致间隔缩短、已掌握词反复重现。

| question_type | 题型 | FAST (ms) | SLOW (ms) |
|---|---|---|---|
| 1 | 英→中 四选一 | 3000 | 8000 |
| 2 | 中→英 四选一 | 3500 | 9000 |
| 3 | 听音辨词 | 4000 | 10000 |
| 4 | 例句挖空 | 5000 | 12000 |
| 5 | 全拼写 | 8000 | 20000 |

**题型加权**：`question_type >= 4` 且答对时 rating 上调一档，封顶 Easy(4)。
**计时起点**：Lv.3 从音频播放结束计，其余从题目渲染完成计。

---

## 6. 题型阶梯（spec F3）

| Lv | 题型 | 解锁条件 | 干扰项来源 |
|---|---|---|---|
| 1 | 英→中 四选一 | 默认 | 同 `pos` 随机 3 个 |
| 2 | 中→英 四选一 | `question_level >= 2` | 同 `pos` + 编辑距离 ≤ 3 优先 |
| 3 | 听音辨词 | `question_level >= 3` | 同 `pos` + 音近（首音素相同）优先 |
| 4 | 例句挖空 | `question_level >= 4` | 同 `pos` + 同 `frequency_band` |
| 5 | 全拼写（首字母提示） | `question_level >= 5` **且 `frequency_band <= 2`** | 无（输入题） |

**Lv.5 准入限制（决议 S10）**：拼写题仅对 `frequency_band` 1–2 的核心词启用；其余词最高阶止于 Lv.4。
故 `QueueItem` 必须携带 `frequency_band`——前端据此判定题型上限。
`QueueItem` 同时携带 `last_review_at`（可空）。前端还原 ts-fsrs Card 时必须写入
`last_review` 与 `elapsed_days`，禁止把已学词当成「刚复习过」（elapsed_days=0），
否则逾期词的可提取度被高估、间隔系统性偏短。

**Lv.3 的音频前置**：听音辨词依赖 TTS。发音未接入前（MOCKS M2）该级降为 Lv.2，
否则用户面对的是无声题面，只能盲猜。降级逻辑在 `src/core/question.ts::effectiveLevel`。
理由——目标用户 ADHD + 基础薄弱，拼写是认知负荷最高、挫败感最强的题型；而产品目标是**词汇量覆盖**（认识词）而非写作产出，高考词汇考查绝大部分是认知性的。为 4800 词全部要求拼写掌握，投入产出比过低。

### 6.1 自由练习的专项模式（F8）

自由练习可强制全部出某一题型：`spelling` 恒为 Lv.5，`dictation` 恒为 Lv.3。
实现在 `src/core/question.ts::drillLevel`，队列、评分与提交路径与普通自由练习完全相同——
专项模式只换考查角度，不是独立玩法。

| 限制 | 自动阶梯 | 专项模式 | 理由 |
|---|---|---|---|
| S10 频段上限 | 生效 | **不生效** | 该限制约束的是「系统擅自把低频词推到最难题型」。专项模式是用户主动选的，选了拼写却收到选择题只会被当成故障 |
| Lv.3 音频前置 | 生效 | **生效** | 无声的听写不是「更难」，是无解。TTS 关闭时听写模式在入口即置灰并说明原因 |

专项模式下的作答同样写入 FSRS 与 `review_logs`：那是真实作答，不设「练习不计数」的旁路——
第二条提交路径意味着第二套未经验证的状态机。

**干扰项分级（决议 S11）**：Lv.1 只用同词性随机，**不引入编辑距离**——`adapt/adopt/adept` 放在一起会对初学者制造**混淆记忆**。形近词精细区分从 Lv.2 起才逐步引入。

**干扰项硬约束**：4 个选项两两不等；干扰项释义不得与正确释义有子串包含关系；候选不足时降级到同 `zone` 随机补足。

---

## 7. XP / 等级 / Streak（spec F6）

```
基础 XP:  Easy=15  Good=10  Hard=5  Again=1
连击倍率: 连对 3-4 次 ×1.2 | 5-7 次 ×1.5 | ≥8 次 ×2.0
等级:     level = floor(sqrt(total_xp / 50)) + 1     -- 上限 100
```

### 7.1 Streak 判定（决议 S1 / S4 / S6 / S8）

spec F6 原规则「三时段全完成才计 1 天」与 §1.3 目标「3 时段至少完成 2 个即达标」直接矛盾——用户达到产品自定义的成功标准仍会断签。修正为：

```
当日 eligible_sessions = 当日实际弹出过（或用户主动发起）的时段数
当日 completed         = is_completed = 1 的时段数

if 今日暂停已激活                     -> FROZEN：streak 不增不减，不计断签
else if eligible_sessions == 0        -> FROZEN：从未弹出（全时段全屏），不计断签 [S6]
else if completed >= 2                -> current_streak += 1
     且 completed == 3                -> 额外触发「完美日」奖励（双倍抽卡券）
else                                  -> 断签：优先自动消耗 makeup_cards，否则 current_streak = 0
```

**关键定义**
- `eligible_sessions`：区分「用户拒绝」与「从未弹出」。全屏导致整个时段静默跳过时不计入分母——**不能惩罚用户未曾获得的机会**。
- **今日暂停 = 冻结**（非补签）：streak 不增不减。与补签卡（修复已断）语义区分。月限 2 次。
- **补签卡**：MVP 阶段每月 1 日自动发放 2 张（不依赖 P1 赛道积分，修 S4）；断签时自动消耗，无需用户操作。

> 现有 `StatsPanel.tsx` 的 `total_xp % 100` 进度条与等级公式不符，需按上式重写。

---

## 8. 词库数据契约

```typescript
interface WordImportDto {
  word: string            // 必填，唯一键，小写
  phonetic: string        // IPA，含 / /
  pos: string             // 受控词表
  meaning: string         // 中文释义，多义用「，」分隔
  pos_2?: string | null   // 第二词性，可空（迁移 016）
  meaning_2?: string | null
  example_1: string       // 必填，游戏/动漫/绘画语境
  example_2: string       // 可空
  level: 'junior' | 'senior' | 'cet4' | 'art'
  frequency_band: 1 | 2 | 3 | 4 | 5
  zone: 'newbie' | 'grass' | 'water' | 'fire' | 'thunder' | 'ice' | 'rock'
}
```

**导入校验（失败即拒，禁止静默跳过）**
- `word` 非空、`^[a-z][a-z\-' ]*$`
- `phonetic` 以 `/` 开头结尾
- `pos` ∈ 受控词表
- `meaning` 非空且不含英文字母（防止字段错位）
- `example_1` 非空且包含 `word` 的某个词形
- `frequency_band` ∈ 1..5，`zone` ∈ 受控词表
- `pos_2` / `meaning_2` 同时有或同时无；`pos_2 ≠ pos`；`meaning_2` ≤ 20 字

**第二词性（迁移 016）**

47% 的词有两种以上词性，教学区间（rank 600–5000）里有 1,599 个实词如此。
单列一个 `pos` 意味着 `watch` 只教「看」不教「手表」、`train` 只教「火车」
不教「训练」、`right` 只教「正确的」不教「权利」——都是高考高频考点。

**为什么另存两列，而不是拼进 `meaning`**：拼在一起会毁掉四选一。只有部分词
有第二词性，正确答案就成了唯一那个「长选项」，不认识单词也能选对；干扰项按
同词性挑，补不齐这个结构。

所以 **出题只用主词性**（选项长度一致），第二词性在答完揭晓时补充展示——
考一个义项，教两个。`null` 就是「没有」，不用空串伪装。

`build_library.py` 的 `SECOND_POS_REQUIRED` 哨兵锁住
watch / train / right / light / park / plant / firm / share：重新生成时若丢了
第二词性，构建当场失败。

**释义从哪来**（2026-08-27 修订）

```
ECDICT csv ──extract.py──────→ words.json      词条 + extract 挑的释义
           └─extract_senses.py→ senses.json     每个词的**全部**释义行
                                    │
                     gen_meanings.py（DeepSeek 挑选）
                                    ↓
                               meanings.json    最终 pos + meaning
                                    │
        words.json + examples.json + meanings.json ──build_library.py──→ library.json
```

`extract.py` 原本先用 `exchange` 的词形变化筛词性、再取该词性的**第一行**释义。
ECDICT 的行序是词典编排顺序而非常用度，于是系统性挑中生僻义——实测前 130 个
高频词约四分之一挑错：

```
can    vt. 装罐              （aux. 能, 可以 在第 3 行）
may    n. 五月               （aux. 可以 在第 2 行）
must   n. 未发酵葡萄汁        （aux. 必须 在第 2 行）
still  n. 蒸馏室, 剧照        （adv. 仍然 在第 4 行）
survey n. 纵览, 视察, 测量    （「调查」是同一行第 5 个义项，被截断切掉）
```

规模：76% 的词有多个词性行，19% 的词 `exchange` 为空、完全没有形态学证据，
29% 的词选中行的义项被截断到 3 个。错的释义伤两次——既当答案错，也当**干扰项**
污染别的题。

`gen_meanings.py` 是**选择题不是生成题**：模型只能从 `senses.json` 的候选行里挑，
校验强制返回的每个义项都是原文子串，挑错顶多是选了次要义项，编不出词库里
没有的东西。被拒的词沿用 `extract.py` 的原值，重跑脚本会只处理它们。

`build_library.py` 带**词性哨兵**（`SPOT_CHECKS`）：can/may/must/will/should/
would/could 必须是 `aux.`，still/just/even/well 必须是 `adv.`，but 必须是
`conj.`。释义重新生成时若又挑回生僻义，构建当场失败——否则要等孩子背错了
才会有人发现。

**分区规则**（2026-08-06 按实测数据修订）

原规则「`zone` 由 `level + frequency_band` 直接推导」在真实词库上失效：格子大小
完全由数据决定，`junior ∧ band1-2` 恰好有 1,271 词（spec §5.2 说 200），
`senior ∧ band5` 只有 128 词（spec 说 500）。spec 那张表总和 2,050，
而实际词库 3,657——它是按更小的词库假想画的。

改为**按难度排序后按比例切分**：

```
排序键：(frequency_band, level, 词频排名)     初中词在同档内优先
newbie  固定前 50 词
其余按 spec §5.2 的比例 4:6:10:10:10 切分
```

实测结果：

| zone | 词数 | 初中词 | 高中词 |
|---|---|---|---|
| newbie | 50 | 50 | 0 |
| grass | 360 | 360 | 0 |
| water | 541 | 389 | 152 |
| fire | 901 | 522 | 379 |
| thunder | 901 | 186 | 715 |
| ice | 904 | 74 | 830 |

难度梯度由排序保证（初中词集中在前段、高中词集中在后段），各区词数回到设计
控制之下。`rock`（美术生专用）暂无数据，保留在受控词表中。

---

### 3.7 能力概览

```rust
/// 水平估到哪、重点该放在哪一段。「学习范围」下拉框的替代品。
get_ability_overview() -> Result<AbilityOverview, String>

pub struct AbilityOverview {
    pub vocabulary: i64,            // 估计词汇量
    pub vocabulary_low: i64,        // ±1 标准误换算成的区间
    pub vocabulary_high: i64,
    pub frontier_from: i64,         // 学习前沿的词频排名区间
    pub frontier_to: i64,
    pub known: i64,                 // 词库按能力分层的词数
    pub frontier: i64,
    pub too_hard: i64,
    pub frontier_untouched: i64,    // 前沿里还没学过的词数
    pub observations: i64,          // 为 0 表示还在用先验，界面须说明
}
```

分层由 `ability::tier` 逐词判定，**不在 SQL 里抄一份边界**——两处各写一份，
改了阈值界面就开始说谎。

## 9. 摸底（spec F5，决议 S2 / S7 已由 §13 取代）

> **只有一个职责：给能力估计一个起点。** 不再逐词预分级。

### 9.0 从「逐词预分级」改成「只定 θ」

原设计考 60 道**初中**词，按频段整段判定：band 1 对 11 题就把那 1067 个词全部
标成「已掌握」，对 9 题则整段降级。60 题产出 5 个桶的结论，覆盖 1600 个词，
而且 senior 与 cet4 一律当新词。它还据此预建约 1438 条 `word_states`，那些词
因此被挡在新词队列之外——依据只是一次频段级的猜测。

这件事现在由 §13 的能力模型做，而且细得多：θ 给出**每个词**的掌握概率，
并且每天的作答都在修正它。

| | 旧 | 新 |
|---|---|---|
| 题量 | 60（5 频段 × 12） | **20** |
| 范围 | 只有 junior | 全库，按词频排名 |
| 产出 | 5 个频段的掌握率 | 一个 θ |
| 副作用 | 预建约 1438 条 `word_states` | **无** |
| 后续修正 | 每场 2 题的抽查层 | 每天的首见作答（§13.3） |

### 9.1 选题：楼梯法

取**离当前能力边界最近、且没问过**的词（`placement_asked` 去重）。那里信息量
最大——答对答错各半，每一题都真正改变估计。问第 1 名的词答对说明不了任何事，
问第 40000 名答对多半是蒙的。

远近按**对数**距离算，因为难度是 `log2(排名)`。用线性的 `ABS(rank - boundary)`
会系统性偏向简单词：边界在第 2500 名时 `|1024−2500| = 1476` 小于
`|4096−2500| = 1596`，于是选了第 1024 名——而 log 尺度上第 4096 名近得多。
实现用 `r/b + b/r`（同增同减，且不需要 SQLite 的数学函数扩展）。

**不受 `study_level` 约束**：那是「想练哪本考纲」，这里在测能力，用全库才准。

### 9.2 为什么只有 20 题

一次性摸底不可能精确——四选一有 25% 的猜对下限，信息量存在上限。模拟
（先验第 2500 名，估计落在真值 ±50% 内的比例）：

```text
真实水平    0 题    12 题   16 题   20 题   24 题
第   800 名   0%     78%     82%     86%     89%
第  1500 名 100%     84%     88%     90%     92%
第  4000 名 100%     80%     90%     93%     96%
第  8000 名   0%     78%     85%     90%     90%
```

0 题（纯先验）时，水平偏离先验的孩子**首场难度必错**。20 题把这件事解决掉，
再加题收益很小。剩下的精度靠日常作答积累——一周的观测量远超任何摸底测试。

摸底期间用 `PLACEMENT_PRIOR_INFORMATION = 0.5`，比日常的 2.0 弱得多：此时
什么都还没教，波动不付代价，只有「多久摸到真实水平」重要。20 题之后信息量
约 3.0，已高于日常先验，之后的更新自然被压住。

**首题之前必须把能力重置到摸底起点**，否则会从一个陈旧的强估计出发，20 题
推不动它，等于白测。

### 9.3 不再写 `word_states`

摸底答对一次不等于掌握。真要跳过某个词，让 θ 去判——它对每个词都有概率，
而且会随作答修正。预建状态是把一次性的猜测**固化**成不可见的过滤条件。

迁移 015 删掉 `word_states WHERE reps = 0`（那些行里没有任何实际观测）并
`DROP TABLE placement_results`。`placement_asked` 保留，用于摸底内去重。

### 9.4 词库规模与吞吐（决议 S2 / S13 实测）

```
730 天 − 90 天末尾巩固期（末批词达 mastered 需 2-3 月）= 有效学习期 640 天
```

**词库实际规模（2026-08-06 实测，决议 S2 验证项）**

```
ECDICT gk（高考考纲）      3,677
ECDICT zk（中考考纲）      1,603
两者交集                   1,554      zk 的 96.9% 已含于 gk
并集去重                   3,726  ←  实际词库规模

摸底判掉约 960 词（中考词掌握率 ~60%）
实际待学 3,726 − 960 = 2,766 词
```

> ⚠️ spec 假设「1600 + 3500 = 5100 词」，把两份词表当作独立集合相加。
> 实测显示中考词汇 **96.9% 已包含在高考考纲内**，并集比假设少 27%。
> 决议 S2 预判到了这一点，验证方式只需一次集合运算。
>
> S2 原定「若实际仅 3500–3800，须把 `daily_new_words` 下调至 4」。该动作
> **不再需要**——S13 已将单场词量提到 20，容量反而过剩：640 天可学约 3,699 词，
> 待学仅 2,766 词。按此进度约 **478 天学完全部新词**，余下约 160 天用于巩固，
> 正好覆盖「末批词达 mastered 需 2–3 月」的要求。

**新词吞吐受复习开销约束（决议 S13，T08 实测）**

```
新词吞吐/天 ≈ 总词次/天 ÷ 9.3
  └ 每学 1 新词产生约 4.7 复习词次 + 3.6 强化词次

3 场 × 20 词 = 60 词次/天 → 实测 5.78 新词/天
640 天 × 5.78 = 3699 词    达成 3840 目标的 96%
```

> ⚠️ 早期版本按「3840 ÷ 640 = 6.0 新词/天」推算，**未计入复习开销**，据此得出的
> 「每场 3–5 词即可」是错的——实测只有 1.62 新词/天。
>
> 此时瓶颈已从排队槽位转移到 `daily_new_words = 6` 本身，继续加大单场词量收益递减。
>
> **9.3 这个系数基于简化 stability 模型（首次 3 天、后续 ×2.5）**。T11 接上 `ts-fsrs`
> 后必须用真算法重测；偏差超 ±20% 则回头修订 S13。

### 9.4.1 会话容量契约

| 项 | 值 | 来源 |
|---|---|---|
| 单场词量 `base_limit` | **20** | 决议 S13（spec §3.1 原为 3–5） |
| 单场时长上限 | **≤240 秒** | 决议 S13（spec §1.3 原为 ≤120 秒） |
| 合并上限 `MERGED_LIMIT` | **30** | 决议 S13（spec F1 原为 8，基于旧的 3–5 词设定） |
| 时段数 | 3（维持不变） | spec F1 |

**中断容忍（S13 配套要求）**：单场延长至约 4 分钟后，中途退出不再是边缘场景。
会话必须支持**随时退出并保留已完成部分**——已 `commit_review` 的词不得回滚，
未作答的词留在队列中由下次会话重新排入。

**T15 强制验证项**：spec 假设「中考 1600 + 高考 3500」去重后仍为 5100，但国内高考考纲 3500 词表**通常已包含**中考 1600 词。拿到真实词表后立即执行 `SELECT COUNT(DISTINCT word)`，若实际仅 3500–3800，`daily_new_words` 默认值下调至 4。

## 10. 抽卡系统（决议 S9，提前进 MVP）

> spec 原将 F12 置于 M3。但 MVP 的长期钩子（家园/赛道/抽卡）全在 P1，只剩 streak+XP，而 streak 又有 S1/S4/S6 三处问题。极简抽卡实现成本远低于家园建造，即时奖励心理效果强。

### 11.1 MVP 范围（仅这些，其余留 P1）

- 完成一个时段 → 获得 1 张抽卡券（`player_stats.draw_tickets += 1`）；「完美日」额外 +1
- 开卡动画 → 随机产出一张卡 → 写入 `card_collection`
- 图鉴页：已收集 / 未收集（剪影）网格，点击看大图 + 一句冷知识
- 重复卡：`count += 1`，暂不转积分（积分兑换属 P1 F11）

### 11.2 卡池与稀有度

```
rarity 1 普通  70%
rarity 2 稀有  25%
rarity 3 传说   5%
```

- **卡池 A `painting`**：世界名画像素化（星月夜、神奈川冲浪里…），**必须为公有领域**
- **卡池 B `creature`**：原创像素生物
- 每张卡的 `source` 字段记录来源 URL 与许可证 —— spec F12 验收项「仓库内附素材来源清单」

### 11.3 素材硬约束

- ❌ 禁止任何商业游戏的角色名、立绘、贴图（spec §4）
- ✅ 仅公有领域（Wikimedia Commons PD 标记）或原创生成
- 像素化处理脚本置于 `scripts/cards/`，原图不入库，仅提交处理后素材 + `SOURCES.md`

### 11.4 Command 签名

```rust
draw_card() -> Result<DrawResult, String>          // 券不足返回 Err，禁止静默失败
get_collection() -> Result<Vec<CollectionEntry>, String>
mark_cards_seen(card_ids: Vec<i64>) -> Result<(), String>   // 清 is_new 红点
```

---

## 11. 测试断言意图（不写测试代码，只定义「断言什么」）

### Rust 侧（`#[cfg(test)]`，用 sqlite in-memory）

| 模块 | 断言意图 |
|---|---|
| `migrations` | 空库跑完 001 后，`PRAGMA table_info` 对每张表列名/类型与本文件 §2 完全一致；重复执行幂等 |
| `queue` | 强化占比达当前 `reinforce_ratio`；强化池为空时不报错且用复习/新词补足 |
| `queue` | **自适应三档**（§4.1）：R=15/16/30/31 四个边界上，新词额度与强化配额取值正确；R 回落后额度自动恢复 |
| `queue` | 摸底词抽查只填充剩余空位——强化+复习占满时，返回结果中摸底词数为 0（§9.2④） |
| `queue` | `due_at > now` 的词不出现在结果中 |
| `streak` | §7.1 五条分支：暂停日冻结 / `eligible_sessions=0` 冻结 / 2 完成→+1 / 3 完成→perfect / 断签优先消耗补签卡 |
| `streak` | 补签卡每月只发放一次（`last_grant_month` 防重复），跨月重置 `pause_used_month` |
| `onboarding` | 摸底判「已会」的词不进新词队列，但 180 天内至少被排队抽查一次（§9.3） |
| `onboarding` | stability 抖动后到期日分散——960 词预分级后，任意单日到期数不超过日词次预算 |
| `cards` | `draw_tickets = 0` 时 `draw_card` 返回 Err 而非静默失败；稀有度分布在 10000 次抽样下符合 70/25/5（±2%） |
| `commit_review` | 一次调用后 `word_states` 与 `review_logs` 同时更新；中途注入错误则两者都不变（事务性） |
| `sessions` | `postpone_session` 第 4 次调用返回 Err；`UNIQUE(date, session_type)` 冲突被正确处理 |
| `stats` | 跨本地午夜的两条 log 被分入不同 `date`；UTC 存储 + 本地日归属正确 |
| `platform` | 非 Windows 下 `user_busy_state()` 返回 `Unknown` 而非 `Normal` |

### 前端侧（vitest）

| 模块 | 断言意图 |
|---|---|
| `autoRating` | §5 **每个题型**各自的 FAST/SLOW 边界值（如 Lv.1 的 2999/3000/7999/8000，Lv.5 的 7999/8000/19999/20000）；Lv≥4 答对上调且不越过 Easy |
| `stateMachine` | §4 表格每一行转移；`reinforcing` 连对 1 次不升级、**第 2 次升级**；8s 外答对清零计数 |
| `stateMachine` | mastered 抽查失败回落 reinforcing 且 `mastered_at` 清空 |
| `distractors` | 4 选项互不相同；干扰项与正确答案无子串包含；候选不足时降级路径被走到且仍返回 4 项 |
| `xp` | 连击倍率三档边界（2/3、4/5、7/8 次）；等级公式在 total_xp=0/50/200/500000 处的取值 |
| `fsrsAdapter` | 同一 (state, rating) 输入下 ts-fsrs 输出被完整映射进 `ReviewCommitDto`，无字段丢失 |

---

## 12. 禁止事项（对应 `strict-execution-rules` §5）

- ❌ 业务代码出现 mock / 硬编码释义数组 / `"选项A"` / 硬编码 `"09:00"`
- ❌ `catch` 后静默 fallback 到本地假数据（审计 D6）
- ❌ 手写日期/日历运算（审计 D2、D3）
- ❌ `// simplified for MVP` / `// In production, would...` / `// In a real app...` 类话术
- ❌ 命令返回 `Ok(())` 但实际什么都没做（审计：`tts.rs`、`trigger_popup_now`）
- ✅ 唯一例外：`// TODO(M5): ...` 形式的占位符，且必须同步登记进 `MOCKS.md`

---

## 13. 能力估计（`src-tauri/src/ability.rs`）

> 取代手选「初中 / 高中 / 四级」。那三个标签和难度基本无关——102 个高中词的
> 常用度和 `the` 同级，28 个初中词比大多数四级词还生僻；用标签筛选既在练已经
> 会的词，又在漏掉该练的词。

### 13.1 模型

词汇掌握对词频高度单调（会 `abandon` 的人几乎必然会 `the`），所以「这孩子会不会
某个没见过的词」可以用一个数回答：他的掌握边界落在词频轴的哪一位。

```text
难度  d = log2(frequency_rank)
能力  θ 同尺度 —— θ = 11 意味着「第 2048 名前后的词有一半把握」
答对  P = c + (1-c)·σ((θ-d)/s)
真会  P_known = σ((θ-d)/s)          ← 内容筛选用这个
```

| 常量 | 值 | 说明 |
|---|---|---|
| `SLOPE` (s) | 1.1 | **未校准**，先验值。决定前沿宽窄，真值须由实际作答估出 |
| `GUESS` (c) | 0.25 | 四选一猜对率。不建模会把蒙对当掌握 |
| `PRIOR_THETA` | log2(2500) | 冷启动先验。低估只是多练已会的词，高估会让孩子一上来就撞墙 |
| `PRIOR_INFORMATION` | 2.0 | 约 16 次观测。由模拟选定，见代码注释里的对照表 |

**内容筛选用 `P_known` 而非 `P`**：后者对完全不会的词也给 0.25，拿它当门槛会把
「四分之一能蒙对」误读成「有点印象」。

### 13.2 分层

| 层 | 判据 | 用途 |
|---|---|---|
| `Known` | `P_known > 0.85` | 不再排入新词队列 |
| `Frontier` | `0.30 ≤ P_known ≤ 0.85` | **该练的就是这些** |
| `TooHard` | `P_known < 0.30` | 暂缓，等边界推过去 |

`frontier_ranks(θ)` 由阈值反解出排名区间，供排队与界面用——两处各写一份数字，
改了阈值界面就开始说谎。

### 13.3 更新时机

**只在首次遇见、且题型为四选一（`question_type ≤ 4`）时更新。**

- 第五次复习 `abandon` 答对，说明的是「这个应用把它教会了」，不是「本来就会」。
  拿复习结果更新 θ 会让估计随训练虚高，然后系统开始跳过它其实没教过的词。
  应用教会的词由 FSRS 逐词跟踪，不走 θ。
- Lv.5 是全拼写，猜对率为 0。同一个模型套上去会把「拼不出来」当成词汇量不足。
- `frequency_rank IS NULL` 的 18 个词跳过，不插补。

更新用 Fisher 记分法：`θ' = θ + score / (information + info_obs)`。**分母是后验
信息量**，用先验会系统性过冲（首步大 24.8%，30 次同向观测后差 40%）。步长因此
自然衰减，不需要人为系数。

与作答落库同一事务：估计更新了但作答没落库（或反之），会让 θ 与观测数对不上，
而这种不一致没有任何东西会报错。

### 13.4 排队如何用它

新词层按能力分层**排序**（不是过滤）：

| 次序 | 层 | 理由 |
|---|---|---|
| 0 | 前沿 | 该教的就是这些 |
| 1 | 太超前 | 前沿学完了才轮到，从最常用的开始，边界自然往外推 |
| 2 | 无排名 | 难度未知 |
| 3 | 大概率已会 | 兜底，避免词池空掉时无词可教 |

**超前优先于已会**：超前的词还值得学，已经会的是纯浪费。

**排序而非过滤**：硬过滤到前沿的话，边界附近的词学完队列就空了，而 θ 本来
就不该随训练漂移。

摸底抽查层（`PROBE_PER_SESSION = 2`）改为**能力采样器**：这些词 `reps = 0`，
答一次就是一次首见观测。按离掌握边界的远近取——第 1 名的词答对说明不了任何事，
第 40000 名答对多半是蒙的，边界附近每题都真正改变估计。

### 13.5 进步如何发生

θ 稳定后学习不会停滞：候选只在**没学过**的词里选，边界附近的词学完了，池子自然
向更难处推进。进步靠词池消耗，不靠 θ 漂移。
