//! 会话排队策略。契约见 contracts-v1.md §3.1 与 §4.1。
//!
//! 强化队列的自适应控制是本模块存在的核心理由。spec 原设计（连续 3 次离队 +
//! 固定 40% 配额）经 180 天蒙特卡洛模拟验证**永不收敛**——清理速度约 1.26 词/天，
//! 而新增速度 4~6 词/天，强化池单调增长直至配额被永久占满、新词进度归零。
//!
//! 修正方案（决议 S3）是两处改动的组合：离队条件放宽到连续 2 次（在
//! `src/core/stateMachine.ts`），以及此处的三档自适应配额。单独任何一项都不够：
//! 只放宽离队余量太小，只降新词则系统长期停在「暂停新词清池」状态、新词进度腰斩。

use crate::db::{clock, repo::settings, Db};
use rusqlite::{Connection, Row};
use serde::Serialize;
use std::collections::HashSet;
use tauri::State;

/// 合法的会话类型。前端传入值必须在此集合内。
const VALID_SESSION_TYPES: [&str; 4] = ["morning", "noon", "evening", "free"];

/// 合并后的单次弹窗词量上限。
///
/// spec F1 原值为 8，那是基于「每场 3-5 词」设定的。单场提到 20 词后（决议 S13）
/// 该上限失去意义，改为 30——留出 10 词余量吸收上一时段的未完成部分。
pub const MERGED_LIMIT: i64 = 30;

/// 单场词量缺省值。前端不传 limit 时从 settings 读取，缺键才用此值。
const DEFAULT_SESSION_WORDS: i64 = 20;

/// 自适应阈值。R 为强化池大小。
const RELAXED_MAX: i64 = 15;
const STRAINED_MAX: i64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct AdaptiveQuota {
    /// 本次可排入的新词上限
    pub new_words: i64,
    /// 强化词应占的比例
    pub reinforce_ratio: f64,
}

/// 按强化池大小计算配额（contracts §4.1）。
///
/// 正常状态（R ≤ 15）完全等同 spec 原体验；三档而非两档是为了阻尼——
/// 两档会在阈值附近反复横跳。
pub fn adaptive_quota(reinforce_pool: i64, configured_new: i64) -> AdaptiveQuota {
    let configured_new = configured_new.max(0);
    if reinforce_pool <= RELAXED_MAX {
        AdaptiveQuota {
            new_words: configured_new,
            reinforce_ratio: 0.40,
        }
    } else if reinforce_pool <= STRAINED_MAX {
        AdaptiveQuota {
            // 向上取整。不用 div_ceil：该方法在 Rust 1.93 仍是 unstable（int_roundings）
            new_words: (configured_new + 1) / 2,
            reinforce_ratio: 0.50,
        }
    } else {
        AdaptiveQuota {
            new_words: 0,
            reinforce_ratio: 0.60,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueSource {
    /// 强化队列，占比由自适应配额决定
    Reinforcing,
    /// due_at 已到的复习词
    DueReview,
    /// 摸底预分级但从未真正作答的词，用于纠正假阳性
    PlacementProbe,
    /// 从未学过的新词
    New,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueueItem {
    pub word_id: i64,
    pub word: String,
    pub phonetic: String,
    pub pos: String,
    pub meaning: String,
    pub example_1: String,
    pub example_2: String,
    /// Lv.5 拼写题的准入判据（决议 S10：仅 1–2 段核心词开放）。
    /// 前端据此决定该词的题型上限，缺了它拼写题就无从限制。
    pub frequency_band: i64,
    pub difficulty: f64,
    pub stability: f64,
    pub due_at: Option<String>,
    pub fsrs_state: i64,
    pub app_state: String,
    pub reps: i64,
    pub lapses: i64,
    pub question_level: i64,
    pub reinforce_streak: i64,
    pub source: QueueSource,
}

const WORD_COLS: &str = "w.id, w.word, w.phonetic, w.pos, w.meaning, w.example_1, w.example_2, \
                          w.frequency_band";
const STATE_COLS: &str = "s.difficulty, s.stability, s.due_at, s.fsrs_state, s.app_state, \
                          s.reps, s.lapses, s.question_level, s.reinforce_streak";

/// 按列名而非位置读取。
///
/// 位置索引在加列时会静默错位——把 `frequency_band` 插进 `WORD_COLS` 后，
/// 后面每个 `row.get(n)` 都读到了邻居的值，且类型恰好兼容时连编译错误都没有。
/// `words.rs::row_to_word` 一直用列名，此处对齐。
fn row_to_item(row: &Row, source: QueueSource) -> rusqlite::Result<QueueItem> {
    Ok(QueueItem {
        word_id: row.get("id")?,
        word: row.get("word")?,
        phonetic: row.get("phonetic")?,
        pos: row.get("pos")?,
        meaning: row.get("meaning")?,
        example_1: row.get("example_1")?,
        example_2: row.get("example_2")?,
        frequency_band: row.get("frequency_band")?,
        difficulty: row.get("difficulty")?,
        stability: row.get("stability")?,
        due_at: row.get("due_at")?,
        fsrs_state: row.get("fsrs_state")?,
        app_state: row.get("app_state")?,
        reps: row.get("reps")?,
        lapses: row.get("lapses")?,
        question_level: row.get("question_level")?,
        reinforce_streak: row.get("reinforce_streak")?,
        source,
    })
}

fn query_with_state(
    conn: &Connection,
    where_clause: &str,
    order_by: &str,
    params: &[&dyn rusqlite::ToSql],
    limit: i64,
    source: QueueSource,
) -> Result<Vec<QueueItem>, String> {
    if limit <= 0 {
        return Ok(Vec::new());
    }
    let sql = format!(
        "SELECT {WORD_COLS}, {STATE_COLS}
         FROM words w JOIN word_states s ON s.word_id = w.id
         WHERE {where_clause}
         ORDER BY {order_by}
         LIMIT {limit}"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("准备排队查询失败: {e}"))?;
    let rows = stmt
        .query_map(params, |r| row_to_item(r, source))
        .map_err(|e| format!("执行排队查询失败: {e}"))?;
    rows.collect::<Result<_, _>>()
        .map_err(|e| format!("读取排队结果失败: {e}"))
}

fn take_new_words(conn: &Connection, limit: i64) -> Result<Vec<QueueItem>, String> {
    if limit <= 0 {
        return Ok(Vec::new());
    }
    let sql = format!(
        "SELECT {WORD_COLS} FROM words w
         WHERE NOT EXISTS (SELECT 1 FROM word_states s WHERE s.word_id = w.id)
         ORDER BY w.frequency_band, w.id
         LIMIT {limit}"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("准备新词查询失败: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(QueueItem {
                word_id: row.get("id")?,
                word: row.get("word")?,
                phonetic: row.get("phonetic")?,
                pos: row.get("pos")?,
                meaning: row.get("meaning")?,
                example_1: row.get("example_1")?,
                example_2: row.get("example_2")?,
                frequency_band: row.get("frequency_band")?,
                difficulty: 0.0,
                stability: 0.0,
                due_at: None,
                fsrs_state: 0,
                app_state: "new".into(),
                reps: 0,
                lapses: 0,
                question_level: 1,
                reinforce_streak: 0,
                source: QueueSource::New,
            })
        })
        .map_err(|e| format!("执行新词查询失败: {e}"))?;
    rows.collect::<Result<_, _>>()
        .map_err(|e| format!("读取新词失败: {e}"))
}

/// 上一时段。用于 spec F1 的未完成合并；morning 无前置（不跨天合并）。
pub fn previous_session_type(session_type: &str) -> Option<&'static str> {
    match session_type {
        "noon" => Some("morning"),
        "evening" => Some("noon"),
        _ => None,
    }
}

/// 计算本次实际词量：上一时段未完成则合并，总量封顶 `MERGED_LIMIT`。
pub fn effective_limit(
    conn: &Connection,
    date: &str,
    session_type: &str,
    base_limit: i64,
) -> Result<i64, String> {
    use crate::db::repo::sessions;

    let Some(prev) = previous_session_type(session_type) else {
        return Ok(base_limit);
    };
    let Some(prev_session) = sessions::find(conn, date, prev)? else {
        // 上一时段从未开始（例如全屏跳过）——不视为「未完成」而合并，
        // 否则用户会在下一时段被加倍的词量迎面砸中（决议 S6 的同源问题）
        return Ok(base_limit);
    };
    if prev_session.is_completed {
        return Ok(base_limit);
    }

    let remaining = (prev_session.planned_count - prev_session.completed_count).max(0);
    Ok((base_limit + remaining).min(MERGED_LIMIT))
}

/// 组装一次会话的词队列。
///
/// 优先级：强化词（按自适应配额）> 到期复习 > 摸底抽查 > 新词。
/// 摸底抽查只填充剩余空位，永不挤压核心学习（契约 §9.2④）。
pub fn build(
    conn: &Connection,
    session_type: &str,
    base_limit: i64,
) -> Result<Vec<QueueItem>, String> {
    use crate::db::repo::word_states;

    let today = clock::today();
    let limit = effective_limit(conn, &today, session_type, base_limit)?;
    if limit <= 0 {
        return Ok(Vec::new());
    }

    let pool = word_states::count_by_app_state(conn, "reinforcing")?;
    let configured_new = settings::get_int(conn, "daily_new_words", 6)?;
    let quota = adaptive_quota(pool, configured_new);
    let now = clock::now();

    let mut items: Vec<QueueItem> = Vec::new();
    let mut seen: HashSet<i64> = HashSet::new();

    // 1. 强化词：按配额取，池不足则全取
    let reinforce_slots = (limit as f64 * quota.reinforce_ratio).ceil() as i64;
    let reinforcing = query_with_state(
        conn,
        "s.app_state = 'reinforcing'",
        // 先到期的优先；同期则接近离队的优先，让它们尽快清出队列
        "s.due_at ASC, s.reinforce_streak DESC",
        &[],
        reinforce_slots,
        QueueSource::Reinforcing,
    )?;
    for it in reinforcing {
        if seen.insert(it.word_id) {
            items.push(it);
        }
    }

    // 2. 到期复习
    let remaining = limit - items.len() as i64;
    let due = query_with_state(
        conn,
        "s.app_state IN ('learning','review','mastered') AND s.due_at <= ?1 AND s.reps > 0",
        "s.due_at ASC",
        &[&now],
        remaining,
        QueueSource::DueReview,
    )?;
    for it in due {
        if seen.insert(it.word_id) && (items.len() as i64) < limit {
            items.push(it);
        }
    }

    // 3. 摸底抽查：预分级但从未真正作答过的词。
    //    优先于新词——验证摸底假阳性比学新词更紧急（决议 S7）。
    let remaining = limit - items.len() as i64;
    let probes = query_with_state(
        conn,
        "s.reps = 0 AND s.app_state != 'new'",
        // 高频词先验证：误判它们的代价最大
        "w.frequency_band ASC, s.due_at ASC",
        &[],
        remaining,
        QueueSource::PlacementProbe,
    )?;
    for it in probes {
        if seen.insert(it.word_id) && (items.len() as i64) < limit {
            items.push(it);
        }
    }

    // 4. 新词：受自适应配额与剩余空位双重限制
    let remaining = (limit - items.len() as i64).min(quota.new_words);
    for it in take_new_words(conn, remaining)? {
        if seen.insert(it.word_id) && (items.len() as i64) < limit {
            items.push(it);
        }
    }

    Ok(items)
}

/// contracts §3.1：返回本次会话的词队列。
///
/// `limit` 省略时从 `settings.session_word_count` 读取——单场词量是产品参数
/// （决议 S13 定为 20），前端不应硬编码。
///
/// 校验在边界处完成：非法 `session_type` 立即拒绝，不静默降级为默认值。
#[tauri::command]
pub fn get_session_queue(
    db: State<Db>,
    session_type: String,
    limit: Option<i64>,
) -> Result<Vec<QueueItem>, String> {
    if !VALID_SESSION_TYPES.contains(&session_type.as_str()) {
        return Err(format!(
            "非法的 session_type `{session_type}`，应为 {VALID_SESSION_TYPES:?} 之一"
        ));
    }

    let conn = db
        .0
        .lock()
        .map_err(|e| format!("获取数据库锁失败: {e}"))?;

    let limit = match limit {
        Some(n) if n > 0 => n,
        Some(n) => return Err(format!("limit 必须为正数，收到 {n}")),
        None => settings::get_int(&conn, "session_word_count", DEFAULT_SESSION_WORDS)?,
    };

    build(&conn, &session_type, limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;
    use crate::db::repo::{sessions, word_states, words};
    use crate::test_support::in_memory_db;

    fn seed(n: usize) -> Connection {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();
        let items: Vec<words::WordImport> = (0..n)
            .map(|i| {
                // 三字母后缀支持 26^3 个词；两字母在 n>676 时会溢出字母表
                let w = format!(
                    "word{}{}{}",
                    (b'a' + ((i / 676) % 26) as u8) as char,
                    (b'a' + ((i / 26) % 26) as u8) as char,
                    (b'a' + (i % 26) as u8) as char,
                );
                words::WordImport {
                    example_1: format!("A {w} appears."),
                    word: w,
                    phonetic: "/w/".into(),
                    pos: "n.".into(),
                    meaning: format!("释义{i}"),
                    example_2: String::new(),
                    level: "junior".into(),
                    frequency_band: 1,
                    zone: "newbie".into(),
                    source_edition: String::new(),
                }
            })
            .collect();
        let out = words::import(&mut conn, &items).unwrap();
        assert!(out.rejected.is_empty(), "夹具词条未通过校验: {:?}", out.rejected);
        conn
    }

    fn set_state(conn: &Connection, id: i64, app_state: &str, due_at: &str, reps: i64) {
        word_states::upsert(
            conn,
            &word_states::WordState {
                word_id: id,
                difficulty: 5.0,
                stability: 1.0,
                due_at: due_at.into(),
                fsrs_state: 1,
                app_state: app_state.into(),
                reps,
                lapses: 0,
                question_level: 1,
                reinforce_streak: 0,
                last_review_at: None,
                mastered_at: None,
            },
        )
        .unwrap();
    }

    fn past() -> String {
        clock::format_ts(clock::parse_ts(&clock::now()).unwrap() - chrono::Duration::days(1))
    }
    fn future() -> String {
        clock::format_ts(clock::parse_ts(&clock::now()).unwrap() + chrono::Duration::days(30))
    }

    // ── 自适应配额（纯函数） ──────────────────────────

    #[test]
    fn 自适应配额在三档边界取值正确() {
        // 宽松档
        assert_eq!(
            adaptive_quota(15, 6),
            AdaptiveQuota { new_words: 6, reinforce_ratio: 0.40 }
        );
        // 越过 15 进入中间档
        assert_eq!(
            adaptive_quota(16, 6),
            AdaptiveQuota { new_words: 3, reinforce_ratio: 0.50 }
        );
        // 中间档上界
        assert_eq!(
            adaptive_quota(30, 6),
            AdaptiveQuota { new_words: 3, reinforce_ratio: 0.50 }
        );
        // 越过 30 进入紧张档：新词归零
        assert_eq!(
            adaptive_quota(31, 6),
            AdaptiveQuota { new_words: 0, reinforce_ratio: 0.60 }
        );
    }

    #[test]
    fn 配额随强化池回落而恢复() {
        assert_eq!(adaptive_quota(40, 6).new_words, 0);
        assert_eq!(adaptive_quota(20, 6).new_words, 3);
        assert_eq!(adaptive_quota(5, 6).new_words, 6, "池子降下来应自动恢复满额");
    }

    #[test]
    fn 中间档新词数向上取整不归零() {
        // 配置为 1 时减半不应变成 0——那等于提前进入紧张档
        assert_eq!(adaptive_quota(20, 1).new_words, 1);
        assert_eq!(adaptive_quota(20, 5).new_words, 3);
        assert_eq!(adaptive_quota(20, 0).new_words, 0);
    }

    // ── 排队 ──────────────────────────

    #[test]
    fn 空库返回空队列而非报错() {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();
        assert!(build(&conn, "morning", 5).unwrap().is_empty());
    }

    #[test]
    fn 全新库只排新词且受配额限制() {
        let conn = seed(20);
        let q = build(&conn, "morning", 5).unwrap();
        assert_eq!(q.len(), 5);
        assert!(q.iter().all(|i| i.source == QueueSource::New));

        // daily_new_words 默认 6，limit=10 时新词仍被配额卡在 6
        let q = build(&conn, "free", 10).unwrap();
        assert_eq!(q.len(), 6, "新词数应受 daily_new_words 限制");
    }

    #[test]
    fn 强化词占比达到配额下限() {
        let conn = seed(30);
        for id in 1..=10 {
            set_state(&conn, id, "reinforcing", &past(), 1);
        }
        let q = build(&conn, "morning", 5).unwrap();
        let reinforcing = q.iter().filter(|i| i.source == QueueSource::Reinforcing).count();
        // R=10 → 宽松档 40%，ceil(5*0.4)=2
        assert!(reinforcing >= 2, "强化占比未达 40%，实际 {reinforcing}/5");
    }

    #[test]
    fn 强化池扩大时配额随之提高() {
        let conn = seed(60);
        for id in 1..=35 {
            set_state(&conn, id, "reinforcing", &past(), 1);
        }
        let q = build(&conn, "morning", 5).unwrap();
        let reinforcing = q.iter().filter(|i| i.source == QueueSource::Reinforcing).count();
        // R=35 → 紧张档 60%，ceil(5*0.6)=3
        assert!(reinforcing >= 3, "紧张档强化占比应达 60%，实际 {reinforcing}/5");
        assert!(
            !q.iter().any(|i| i.source == QueueSource::New),
            "紧张档不应排入新词"
        );
    }

    #[test]
    fn 强化池为空时用其他来源补足不报错() {
        let conn = seed(20);
        set_state(&conn, 1, "review", &past(), 3);
        let q = build(&conn, "morning", 5).unwrap();
        assert_eq!(q.len(), 5);
        assert!(!q.iter().any(|i| i.source == QueueSource::Reinforcing));
    }

    #[test]
    fn 未到期的复习词不出现在队列中() {
        let conn = seed(3);
        for id in 1..=3 {
            set_state(&conn, id, "review", &future(), 3);
        }
        let q = build(&conn, "morning", 5).unwrap();
        assert!(
            !q.iter().any(|i| i.source == QueueSource::DueReview),
            "due_at 在未来的词不应被排入"
        );
    }

    #[test]
    fn 摸底词优先于新词填补空位() {
        let conn = seed(20);
        // 1-3 号是摸底预分级词（reps=0，状态非 new，due 在未来）
        for id in 1..=3 {
            set_state(&conn, id, "review", &future(), 0);
        }
        let q = build(&conn, "morning", 5).unwrap();

        let probes = q.iter().filter(|i| i.source == QueueSource::PlacementProbe).count();
        assert_eq!(probes, 3, "3 个摸底词都应被排入");
        assert_eq!(q.len(), 5);

        // 摸底词排在新词之前
        let first_probe = q.iter().position(|i| i.source == QueueSource::PlacementProbe);
        let first_new = q.iter().position(|i| i.source == QueueSource::New);
        assert!(
            first_probe < first_new,
            "摸底抽查应优先于新词——验证假阳性比学新词更紧急"
        );
    }

    #[test]
    fn 强化配额占满全部空位时摸底抽查让位() {
        let conn = seed(50);
        for id in 1..=3 {
            set_state(&conn, id, "review", &future(), 0);
        }
        // R=36 → 紧张档，ratio=0.60；limit=2 时 ceil(2*0.6)=2，空位被占满
        for id in 10..=45 {
            set_state(&conn, id, "reinforcing", &past(), 1);
        }

        let q = build(&conn, "morning", 2).unwrap();
        assert_eq!(q.len(), 2);
        assert!(
            q.iter().all(|i| i.source == QueueSource::Reinforcing),
            "强化配额占满 limit 时不应排入其他来源，实际: {:?}",
            q.iter().map(|i| i.source).collect::<Vec<_>>()
        );
    }

    #[test]
    fn 队列内无重复词条() {
        let conn = seed(20);
        for id in 1..=5 {
            set_state(&conn, id, "reinforcing", &past(), 1);
        }
        let q = build(&conn, "morning", 8).unwrap();
        let unique: HashSet<i64> = q.iter().map(|i| i.word_id).collect();
        assert_eq!(unique.len(), q.len(), "队列中出现重复词条");
    }

    #[test]
    fn 答过的词按_due_at_重新出现() {
        // 审计 D1：原实现只捞 state='new' 的词，答过一次的词永不重现
        let conn = seed(5);
        set_state(&conn, 1, "review", &past(), 3);

        let q = build(&conn, "morning", 5).unwrap();
        assert!(
            q.iter().any(|i| i.word_id == 1 && i.source == QueueSource::DueReview),
            "已到期的复习词未重新出现——审计 D1 未修复"
        );
    }

    // ── 时段合并（spec F1） ──────────────────────────

    #[test]
    fn 上一时段未完成则合并() {
        let conn = seed(30);
        let d = clock::today();

        let s = sessions::start(&conn, &d, "morning", 5, &clock::now()).unwrap();
        assert_eq!(effective_limit(&conn, &d, "noon", 5).unwrap(), 10);

        // 完成 morning 后不再合并
        sessions::finish(&conn, s.id, 5, 50, &clock::now()).unwrap();
        assert_eq!(effective_limit(&conn, &d, "noon", 5).unwrap(), 5);
    }

    #[test]
    fn 合并量封顶在_merged_limit() {
        let conn = seed(60);
        let d = clock::today();

        // 上一时段计划 20 词一个没做，本时段也是 20 词 → 40，应被截到 30
        sessions::start(&conn, &d, "morning", 20, &clock::now()).unwrap();
        assert_eq!(
            effective_limit(&conn, &d, "noon", 20).unwrap(),
            MERGED_LIMIT
        );
    }

    #[test]
    fn 部分完成只合并剩余部分() {
        let conn = seed(30);
        let d = clock::today();
        sessions::start(&conn, &d, "morning", 5, &clock::now()).unwrap();
        conn.execute(
            "UPDATE sessions SET completed_count = 3 WHERE date = ?1 AND session_type = 'morning'",
            [&d],
        )
        .unwrap();

        // 剩余 2 词并入，5 + 2 = 7
        assert_eq!(effective_limit(&conn, &d, "noon", 5).unwrap(), 7);
    }

    #[test]
    fn 从未开始的时段不触发合并() {
        let conn = seed(30);
        let d = clock::today();
        // morning 从未开始（例如整个时段都在全屏游戏中）
        assert_eq!(
            effective_limit(&conn, &d, "noon", 5).unwrap(),
            5,
            "从未弹出的时段不应导致下一时段词量翻倍"
        );
    }

    #[test]
    fn 早间时段无前置不合并() {
        let conn = seed(30);
        let d = clock::today();
        assert_eq!(previous_session_type("morning"), None);
        assert_eq!(effective_limit(&conn, &d, "morning", 5).unwrap(), 5);
    }

    #[test]
    fn 合并链条覆盖三个时段() {
        assert_eq!(previous_session_type("noon"), Some("morning"));
        assert_eq!(previous_session_type("evening"), Some("noon"));
        assert_eq!(previous_session_type("free"), None, "自由练习不参与合并");
    }

    // ── 强化池收敛回归（决议 S3） ──────────────────────────

    /// 确定性伪随机。测试必须可复现，故不引入 rand。
    struct Lcg(u64);
    impl Lcg {
        fn next_f64(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
        }
    }

    /// 简化的 stability 增长，用于替代 FSRS（真实 FSRS 在前端，ADR-2）。
    ///
    /// 首次成功给 3 天、后续 ×2.5，粗略贴合 FSRS 默认权重下的早期增长节奏。
    /// 这个量级很关键：增长过慢会让复习队列虚假积压，把新词全部挤出，得出
    /// 「新词进度崩溃」的错误结论。
    fn grow_stability(current: f64) -> f64 {
        if current <= 1.0 {
            3.0
        } else {
            current * 2.5
        }
    }

    /// 按 contracts §4 转移表推进一个词的状态。
    ///
    /// 本测试验证的是**队列是否收敛**，取决于转移规则与配额，而非间隔的精确数值。
    fn apply_transition(conn: &Connection, item: &QueueItem, correct: bool, fast: bool) {
        let mut s = word_states::get(conn, item.word_id)
            .unwrap()
            .unwrap_or(word_states::WordState {
                word_id: item.word_id,
                difficulty: 5.0,
                stability: 1.0,
                due_at: clock::now(),
                fsrs_state: 0,
                app_state: "new".into(),
                reps: 0,
                lapses: 0,
                question_level: 1,
                reinforce_streak: 0,
                last_review_at: None,
                mastered_at: None,
            });

        s.reps += 1;

        if !correct {
            // 任意状态答错 → 强化队列，计数清零
            s.app_state = "reinforcing".into();
            s.reinforce_streak = 0;
            s.lapses += 1;
            s.stability = 1.0;
            s.question_level = (s.question_level - 1).max(1);
            s.due_at = clock::due_in_days(1.0);
        } else if s.app_state == "reinforcing" {
            if fast {
                s.reinforce_streak += 1;
                // 决议 S3：连续 2 次 8 秒内答对即离队（spec 原为 3 次）
                if s.reinforce_streak >= 2 {
                    s.app_state = "review".into();
                    s.reinforce_streak = 0;
                    s.stability = grow_stability(s.stability);
                    s.due_at = clock::due_in_days(s.stability);
                } else {
                    s.due_at = clock::due_in_days(1.0);
                }
            } else {
                s.reinforce_streak = 0;
                s.due_at = clock::due_in_days(1.0);
            }
        } else {
            s.stability = grow_stability(s.stability);
            s.question_level = (s.question_level + 1).min(5);
            s.due_at = clock::due_in_days(s.stability);

            // 掌握判定必须与前端 stateMachine.ts 一致：稳定性超过 60 天
            // 且通过高阶题型。此处若缺这一分支，模拟里永远不会出现已掌握词，
            // 「已掌握占比」之类的结论便完全失真
            const MASTERY_STABILITY_DAYS: f64 = 60.0;
            const MASTERY_MIN_QUESTION_LEVEL: i64 = 4;
            if s.stability > MASTERY_STABILITY_DAYS
                && item.question_level >= MASTERY_MIN_QUESTION_LEVEL
            {
                s.app_state = "mastered".into();
                if s.mastered_at.is_none() {
                    s.mastered_at = Some(clock::now());
                }
            } else if s.app_state != "mastered" {
                s.app_state = "review".into();
            }
        }

        word_states::upsert(conn, &s).unwrap();
    }

    struct SimResult {
        new_words: usize,
        due: usize,
        reinforce: usize,
        final_pool: i64,
        peak_pool: i64,
    }

    /// 跑 `days` 天模拟，每天 `sessions` 场、每场 `per_session` 词。
    fn simulate(days: usize, sessions: usize, per_session: i64, seed_words: usize) -> SimResult {
        let conn = seed(seed_words);
        let mut rng = Lcg(0x5EED_1234);
        let mut r = SimResult {
            new_words: 0,
            due: 0,
            reinforce: 0,
            final_pool: 0,
            peak_pool: 0,
        };

        for _day in 0..days {
            for _ in 0..sessions {
                // 用 free 规避时段合并逻辑——合并已由独立测试覆盖
                let q = build(&conn, "free", per_session).unwrap();
                for item in &q {
                    match item.source {
                        QueueSource::New => r.new_words += 1,
                        QueueSource::Reinforcing => r.reinforce += 1,
                        QueueSource::DueReview | QueueSource::PlacementProbe => r.due += 1,
                    }
                    // 答对率：新词最低，复习最高，强化居中
                    let p = match item.source {
                        QueueSource::New => 0.65,
                        QueueSource::Reinforcing => 0.80,
                        _ => 0.85,
                    };
                    let correct = rng.next_f64() < p;
                    // 答对者中约 85% 在 8 秒内完成
                    let fast = rng.next_f64() < 0.85;
                    apply_transition(&conn, item, correct, fast);
                }
            }

            // 时间前进一天 ≡ 所有到期日后退一天
            conn.execute(
                "UPDATE word_states SET due_at = strftime('%Y-%m-%dT%H:%M:%SZ', due_at, '-1 day')",
                [],
            )
            .unwrap();

            r.peak_pool = r
                .peak_pool
                .max(word_states::count_by_app_state(&conn, "reinforcing").unwrap());
        }
        r.final_pool = word_states::count_by_app_state(&conn, "reinforcing").unwrap();
        r
    }

    /// 180 天模拟：验证强化池不发散。
    ///
    /// spec 原设计（连续 3 次离队 + 固定 40% 配额）在同等参数下池子会涨到 200+，
    /// 40% 配额被永久占满、新词进度归零。本测试锁定修正方案的收敛性，防止有人
    /// 把 §4.1 的三档自适应改回固定配额。
    #[test]
    fn 强化池在一百八十天模拟中保持收敛() {
        let r = simulate(180, 3, 5, 1300);

        assert!(
            r.final_pool < 40,
            "强化池未收敛：180 天后仍有 {} 词（峰值 {}）",
            r.final_pool,
            r.peak_pool
        );
        assert!(
            r.peak_pool < 80,
            "强化池峰值过高（{}），自适应控制未生效",
            r.peak_pool
        );
    }

    /// 已掌握词会不会挤占日常练习。
    ///
    /// 担忧是合理的：`mastered` 词并非彻底离场，而是进入低频抽查（spec F2）。
    /// 词库 3657 词若最终大半进入该状态，即便每词 60 天才到期一次，
    /// 每天的抽查量也可能压过新词与复习。
    ///
    /// 本测试跑满两年学习周期，统计稳态下各来源的实际占比。
    #[test]
    fn 已掌握词不会挤占日常练习() {
        let conn = seed(3657);
        let mut rng = Lcg(0xC0FF_EE01);

        let (mut n_new, mut n_due, mut n_reinforce, mut n_probe) = (0usize, 0usize, 0usize, 0usize);
        // 只统计后半程——前期几乎没有已掌握词，会稀释稳态占比。
        // 按 app_state 而非 source 统计：已掌握词到期后走的是 DueReview 来源，
        // 与普通复习词混在一起，只有看状态才分得出
        let (mut late_mastered, mut late_total) = (0usize, 0usize);

        for day in 0..730 {
            for _ in 0..3 {
                let q = build(&conn, "free", 20).unwrap();
                for item in &q {
                    match item.source {
                        QueueSource::New => n_new += 1,
                        QueueSource::DueReview => n_due += 1,
                        QueueSource::Reinforcing => n_reinforce += 1,
                        QueueSource::PlacementProbe => n_probe += 1,
                    }
                    if day >= 365 {
                        late_total += 1;
                        if item.app_state == "mastered" {
                            late_mastered += 1;
                        }
                    }

                    let p = match item.source {
                        QueueSource::New => 0.65,
                        QueueSource::Reinforcing => 0.80,
                        _ => 0.88,
                    };
                    let correct = rng.next_f64() < p;
                    let fast = rng.next_f64() < 0.85;
                    apply_transition(&conn, item, correct, fast);
                }
            }
            conn.execute(
                "UPDATE word_states SET due_at = strftime('%Y-%m-%dT%H:%M:%SZ', due_at, '-1 day')",
                [],
            )
            .unwrap();
        }

        let mastered = word_states::count_by_app_state(&conn, "mastered").unwrap();
        let learned = word_states::count_by_app_state(&conn, "review").unwrap() + mastered;
        let total = n_new + n_due + n_reinforce + n_probe;

        println!("\n─── 两年模拟（3657 词库，每天 3 场 × 20 词）───");
        println!("累计词次 {total}");
        println!("  新词      {n_new:>7}  {:>5.1}%", n_new as f64 / total as f64 * 100.0);
        println!("  到期复习  {n_due:>7}  {:>5.1}%", n_due as f64 / total as f64 * 100.0);
        println!("  强化      {n_reinforce:>7}  {:>5.1}%", n_reinforce as f64 / total as f64 * 100.0);
        println!("  抽查      {n_probe:>7}  {:>5.1}%", n_probe as f64 / total as f64 * 100.0);
        println!("学过 {learned} 词，其中已掌握 {mastered}");

        let mastered_ratio = late_mastered as f64 / late_total.max(1) as f64;
        println!(
            "第二年出题中已掌握词占比 {:.1}%（{late_mastered}/{late_total}）",
            mastered_ratio * 100.0
        );

        // 已掌握词的到期间隔以月计，即便积累到数千词，每天到期的也只是其中一小部分。
        // 占比失控意味着掌握判定的稳定性门槛或 FSRS 间隔设置出了问题——
        // 学习者会感到"一直在复习已经会了的词"
        assert!(
            mastered_ratio < 0.5,
            "第二年出题中已掌握词占 {:.1}%，正在挤占新词与复习",
            mastered_ratio * 100.0
        );
    }

    /// 新词吞吐量与单场词量的关系。
    ///
    /// **这个测试锁定的是一个已知的容量约束，不是 bug**：新词在排队优先级中排最后，
    /// 每学 1 个新词会产生约 4.7 个复习词次 + 3.6 个强化词次，即约 9.3 倍的后续负担。
    /// 因此每天的新词吞吐 ≈ 每天总词次 / 9.3。
    ///
    /// contracts §9.1 按「4800 词 ÷ 640 天 = 6 新词/天」推算周期时**未计入这项开销**，
    /// 而 spec §3.1 的「每场 3-5 词」只能提供 15 词次/天 → 实际约 1.6 新词/天。
    /// 详见 spec-review 决议 S13。
    #[test]
    fn 新词吞吐量随单场词量线性变化() {
        let mut rows = Vec::new();
        for per_session in [5i64, 10, 15, 20] {
            let r = simulate(180, 3, per_session, 2600);
            let total = r.new_words + r.due + r.reinforce;
            rows.push((per_session, r, total));
        }

        println!("\n─── 新词吞吐量 vs 单场词量（180 天 × 3 场/天）───");
        println!("单场词量  总词次/天  新词/天  复习/天  强化/天  强化池峰值");
        for (per, r, total) in &rows {
            println!(
                "{per:>6}  {:>9.1}  {:>7.2}  {:>7.1}  {:>7.1}  {:>10}",
                *total as f64 / 180.0,
                r.new_words as f64 / 180.0,
                r.due as f64 / 180.0,
                r.reinforce as f64 / 180.0,
                r.peak_pool,
            );
        }
        let per_day = |r: &SimResult| r.new_words as f64 / 180.0;
        println!(
            "\n要达到 contracts §9.1 假设的 6 新词/天，需单场约 {:.0} 词",
            5.0 * 6.0 / per_day(&rows[0].1)
        );

        // 单场 5 词时新词吞吐远低于契约假设——锁定该事实，避免被无声接受
        assert!(
            per_day(&rows[0].1) < 2.5,
            "单场 5 词的新词吞吐意外偏高（{:.2}/天），模型或实现已变化，需重新核对 S13",
            per_day(&rows[0].1)
        );
        // 吞吐随词量单调上升，说明瓶颈确实是槽位而非其他
        assert!(
            per_day(&rows[3].1) > per_day(&rows[0].1) * 2.0,
            "增大单场词量未显著提升新词吞吐，瓶颈判断有误"
        );
        // 各档强化池都不发散
        for (per, r, _) in &rows {
            assert!(
                r.peak_pool < 120,
                "单场 {per} 词时强化池峰值 {} 过高",
                r.peak_pool
            );
        }
    }
}
