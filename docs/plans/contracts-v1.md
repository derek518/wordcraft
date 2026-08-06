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
  level           TEXT NOT NULL,                    -- 'junior' | 'senior' | 'art'
  frequency_band  INTEGER NOT NULL,                 -- 1..5, 1 = 最高频
  zone            TEXT NOT NULL,                    -- 'newbie'|'grass'|'water'|'fire'|'thunder'|'ice'|'rock'
  created_at      TEXT NOT NULL
);
CREATE INDEX idx_words_zone_band ON words(zone, frequency_band);
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
  vocab_estimate    INTEGER NOT NULL DEFAULT 0,     -- 摸底测试产出
  makeup_cards      INTEGER NOT NULL DEFAULT 0,     -- 补签卡，每月 1 日自动发 2 张（S4）
  pause_used_month  INTEGER NOT NULL DEFAULT 0,     -- 今日暂停已用次数，每月限 2
  draw_tickets      INTEGER NOT NULL DEFAULT 0,     -- 抽卡券（S9）
  last_grant_month  TEXT                            -- 上次发放补签卡的月份 'YYYY-MM'，防重复发放
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
| `daily_new_words` | `"6"` | spec §7.2；**运行时受 §4.1 自适应调制**，此为上限而非定值。T15 验证真实词表规模后可能下调至 4 |
| `placement_stage` | `"0"` | 摸底进度：0=未开始 1=进行中 2=已完成（支持分两次） |
| `daily_pause_date` | `""` | 今日暂停激活的日期，空串表示未激活 |
| `session_word_count` | `"20"` | 单场词量（migration 002，决议 S13）。**前端不得硬编码**，`get_session_queue` 省略 `limit` 时由后端读取 |
| `sound_enabled` | `"true"` | |
| `autostart_enabled` | `"true"` | |
| `tts_provider` | `"edge"` | `edge` \| `sapi` \| `off` |

---

## 3. Tauri Command 签名契约

> 命名规则：`snake_case`（Rust 侧），前端 `invoke` 传参用 `camelCase`（Tauri 2 自动转换）。
> 所有命令返回 `Result<T, String>`；错误必须携带可诊断信息，**禁止吞错返回 `Ok(())`**。

### 3.1 词库 / 排队

```rust
/// 按 §4.1 自适应配额返回本次 session 的词。
/// 排队优先级：强化中 > 到期复习(due_at<=now) > 摸底抽查 > 新词(受 daily_new_words 限额)
/// limit 省略时读 settings.session_word_count（默认 20，决议 S13）
get_session_queue(session_type: String, limit: Option<i64>) -> Result<Vec<QueueItem>, String>

/// 返回指定词的干扰项候选池（同词性、编辑距离近的词），由前端组题。
get_distractor_pool(word_id: i64, pos: String, count: i64) -> Result<Vec<String>, String>

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
/// 返回下一道摸底题；自适应二分，内部维护层级游标
get_placement_question() -> Result<Option<PlacementQuestion>, String>   // None = 测试结束
submit_placement_answer(word_id: i64, is_correct: bool, reaction_ms: i64) -> Result<(), String>

/// 结算：按层掌握率批量预分级（含分层抖动），返回词汇量估计
finalize_placement() -> Result<PlacementResult, String>
```

### 3.4 统计

```rust
get_today_stats() -> Result<TodayStats, String>
get_overall_stats() -> Result<OverallStats, String>
get_mastery_distribution() -> Result<MasteryDistribution, String>  // 五段色条
get_heatmap(days: i64) -> Result<Vec<HeatmapCell>, String>
export_data_json() -> Result<String, String>                       // spec F7 导出
```

### 3.5 系统集成（平台抽象，ADR-3）

```rust
get_next_session_time() -> Result<SessionTime, String>   // 真实计算，禁止硬编码
trigger_popup_now() -> Result<(), String>
set_autostart(enabled: bool) -> Result<(), String>
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
| `new` | 任意作答 | `learning` | `question_level = 1` |
| 任意 | 答错 | `reinforcing` | `reinforce_streak = 0`；当场排入本 session 队尾；`question_level` 降 1（最低 1） |
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
理由——目标用户 ADHD + 基础薄弱，拼写是认知负荷最高、挫败感最强的题型；而产品目标是**词汇量覆盖**（认识词）而非写作产出，高考词汇考查绝大部分是认知性的。为 4800 词全部要求拼写掌握，投入产出比过低。

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
  example_1: string       // 必填，游戏/动漫/绘画语境
  example_2: string       // 可空
  level: 'junior' | 'senior' | 'art'
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

**分区规则**：`zone` 由 `level + frequency_band` 推导 —— newbie=junior∧band1 前 50；grass=junior∧band1-2；water=junior∧band3-5；fire=senior∧band1-2；thunder=senior∧band3-4；ice=senior∧band5；rock=art。

---

## 9. 摸底分级（spec F5，决议 S2 / S7）

> 目标：2 年周期内覆盖全部词汇。摸底的作用是**压缩实际待学量**，不是给每个词打标签。

### 9.1 数学前提

```
730 天 − 90 天末尾巩固期（末批词达 mastered 需 2-3 月）= 有效学习期 640 天
摸底判掉约 960 词（初中 1600 词掌握率 ~60%）
待学 4800 − 960 = 3840 词
```

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

### 9.1.1 会话容量契约

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

### 9.2 四阶段流程

**① 粗筛（无用户感知）**
- `level='senior'` 的词**不进摸底**，默认全部 `new`（新高一大概率未系统学过）
- `level='art'` 的 300 词默认锁定，不占前期额度，随 zone `rock` 解锁
- 摸底范围仅 `level='junior'` 的约 1600 词 → 测试范围缩小 70%，同样 5 分钟得到细得多的粒度

**② 自适应摸底（~60 题 / 5 分钟，可分两次）**
- 1600 词按 `frequency_band` 分 5 层，每层约 12 题，二分查找掌握边界
- 题型固定 Lv.1 英→中四选一 + 记录反应时间
- 连续 3 题错或超时 → 下跳一层
- **产出：每层掌握率 p₁..p₅ + `vocab_estimate`**，而非逐词判定（60 题无法覆盖 1600 词）

**③ 批量预分级（无用户感知）**

| 层掌握率 | app_state | question_level | stability 初值 |
|---|---|---|---|
| p > 0.85 | `review` | 2 | 按 band 分层抖动（见下） |
| 0.5 < p ≤ 0.85 | `learning` | 1 | 3 天 |
| p ≤ 0.5 | `new` | 1 | 0（正常排队） |

**分层抖动（必需）**——若 960 词全赋 stability=14，14 天后集中到期会淹没每日 20 词次预算：

```
band 1-2 (高频核心): stability = uniform(7, 30)    -- 价值最高，早验证
band 3-4:            stability = uniform(30, 90)
band 5   (低频):     stability = uniform(90, 180)
```

**④ 抽查纠错（持续，融入日常）**
- 摸底词在排队中的优先级：`强化词 > 到期复习 > 摸底词抽查 > 新词`
- 抽查**填充剩余空位，不占固定额度**，永不挤压核心学习
- 抽查答错 → 回落 `learning`，假阳性被自动纠正

### 9.3 防猜机制（S7）

四选一有 25% 基础猜对率，基础薄弱者实测常达 30–40%。

```
判定「已会」需同时满足:  is_correct = true  AND  reaction_ms < 4000
```

且 stability 起始值取 7–180 天区间（spec 原为固定 30），**让假阳性在首次抽查时暴露，而非一个月后**。

**测试断言**：摸底判定已会的词不进入新词队列（spec F5 验收项）；但必须能在抽查中出现——断言「已会词在 180 天内至少被排队一次」。

---

## 10. 抽卡系统（决议 S9，提前进 MVP）

> spec 原将 F12 置于 M3。但 MVP 的长期钩子（家园/赛道/抽卡）全在 P1，只剩 streak+XP，而 streak 又有 S1/S4/S6 三处问题。极简抽卡实现成本远低于家园建造，即时奖励心理效果强。

### 10.1 MVP 范围（仅这些，其余留 P1）

- 完成一个时段 → 获得 1 张抽卡券（`player_stats.draw_tickets += 1`）；「完美日」额外 +1
- 开卡动画 → 随机产出一张卡 → 写入 `card_collection`
- 图鉴页：已收集 / 未收集（剪影）网格，点击看大图 + 一句冷知识
- 重复卡：`count += 1`，暂不转积分（积分兑换属 P1 F11）

### 10.2 卡池与稀有度

```
rarity 1 普通  70%
rarity 2 稀有  25%
rarity 3 传说   5%
```

- **卡池 A `painting`**：世界名画像素化（星月夜、神奈川冲浪里…），**必须为公有领域**
- **卡池 B `creature`**：原创像素生物
- 每张卡的 `source` 字段记录来源 URL 与许可证 —— spec F12 验收项「仓库内附素材来源清单」

### 10.3 素材硬约束

- ❌ 禁止任何商业游戏的角色名、立绘、贴图（spec §4）
- ✅ 仅公有领域（Wikimedia Commons PD 标记）或原创生成
- 像素化处理脚本置于 `scripts/cards/`，原图不入库，仅提交处理后素材 + `SOURCES.md`

### 10.4 Command 签名

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
