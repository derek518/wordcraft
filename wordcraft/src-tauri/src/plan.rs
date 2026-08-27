//! 每日学习量的唯一旋钮：**每天学多少个新词**。
//!
//! ## 为什么只留一个
//!
//! 先前有两个设置：`daily_new_words`（实为每场）与 `session_word_count`（单场题数）。
//! 两者之间存在物理约束——每个新词当天还会带来若干次复习，单场题数不可能
//! 独立于新词量取值。把两个都交给用户，等于允许配出自相矛盾的组合：
//! 「每场 40 题、每天 3 个新词」时那 37 题无处可来，队列静静地给不满，
//! 而界面上看不出任何异常。
//!
//! 现在 `daily_new_words` 是**真正的每日预算**，单场题数由它推算。
//!
//! ## 预算怎么分到各时段
//!
//! 按剩余时段均分，而不是固定每场三分之一：跳过早上的话，中午和晚上会
//! 自动各领一半，当天预算仍然走得完。已经学过的新词从预算里扣除，
//! 所以自由练习不会让当天新词翻倍。

use rusqlite::Connection;

use crate::db::clock;
use crate::db::repo::settings;

/// 设置键。语义已从「每场」改为「每日」，迁移 012 负责换算旧值。
pub const SETTING_KEY: &str = "daily_new_words";

/// 缺省每日新词数。等于旧版「每场 6 个 × 3 个时段」，行为不变。
pub const DEFAULT: i64 = 18;

/// 一天的固定时段数（早/中/晚）。
pub const SESSIONS_PER_DAY: i64 = 3;

/// 单场题数下限。预算用尽的时段仍要能复习，不能缩成空场。
const MIN_SESSION_WORDS: i64 = 12;

/// 单场题数上限。目标用户有 ADHD 特征，单场超过这个数就该拆场而不是硬塞。
const MAX_SESSION_WORDS: i64 = 40;

/// 每个新词平均占用的题数（首次学习 + 当天的后续复习）。
///
/// FSRS 的学习阶段会让新词在同一天内再出现一到两次，加上首次学习，
/// 三倍是实测的稳态比例。这个系数把「新词预算」翻译成「单场题数」。
const WORDS_PER_NEW: i64 = 3;

/// 一次会话的学习量安排。全部由每日预算推算，无第二个设置项。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SessionPlan {
    /// 每日新词预算（设置值）
    pub daily_budget: i64,
    /// 今天已经学过的新词数
    pub introduced_today: i64,
    /// 本场可排入的新词上限
    pub new_quota: i64,
    /// 本场题数
    pub session_words: i64,
}

/// 该时段之后（含自身）当天还剩几个时段。
///
/// 自由练习与魔王战不属于三时段之一，按「最后一场」处理：把当天剩余
/// 预算一次给足，否则临时加练永远只能拿到三分之一。
fn sessions_left(session_type: &str) -> i64 {
    match session_type {
        "morning" => SESSIONS_PER_DAY,
        "noon" => SESSIONS_PER_DAY - 1,
        _ => 1,
    }
}

/// 今天首次作答的词数——即「今天学了几个新词」。
///
/// 判据是该词的最早一条作答日志落在今天，而不是 `word_states` 的存在性：
/// 摸底会预建上千条 `word_states`，用它计数会把整个词库算成今天学的。
pub fn introduced_on(conn: &Connection, date: &str) -> Result<i64, String> {
    let (start, end) = clock::local_day_bounds(date)?;
    conn.query_row(
        "SELECT COUNT(*) FROM (
           SELECT word_id, MIN(reviewed_at) AS first_at
           FROM review_logs GROUP BY word_id
         ) WHERE first_at >= ?1 AND first_at < ?2",
        [&start, &end],
        |r| r.get(0),
    )
    .map_err(|e| format!("统计今日新词数失败: {e}"))
}

/// 每日新词预算。非法值回落到缺省，不让一个坏设置停掉学习。
pub fn daily_budget(conn: &Connection) -> Result<i64, String> {
    Ok(settings::get_int(conn, SETTING_KEY, DEFAULT)?.max(0))
}

/// 由每日预算推算本场安排。
pub fn for_session(
    conn: &Connection,
    date: &str,
    session_type: &str,
) -> Result<SessionPlan, String> {
    let daily_budget = daily_budget(conn)?;
    let introduced_today = introduced_on(conn, date)?;
    Ok(compute(daily_budget, introduced_today, session_type))
}

/// 纯函数部分，便于直接测边界。
pub fn compute(daily_budget: i64, introduced_today: i64, session_type: &str) -> SessionPlan {
    let remaining = (daily_budget - introduced_today).max(0);
    let left = sessions_left(session_type);
    // 向上取整分摊。不用 div_ceil：该方法在 Rust 1.93 仍是 unstable（int_roundings）
    let new_quota = (remaining + left - 1) / left;
    let session_words = (new_quota * WORDS_PER_NEW).clamp(MIN_SESSION_WORDS, MAX_SESSION_WORDS);

    SessionPlan {
        daily_budget,
        introduced_today,
        new_quota,
        session_words,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;
    use crate::db::repo::review_logs::{self, NewReviewLog};
    use crate::db::repo::words;
    use crate::test_support::in_memory_db;

    fn plan(budget: i64, done: i64, st: &str) -> SessionPlan {
        compute(budget, done, st)
    }

    #[test]
    fn 早场领三分之一_晚场领全部剩余() {
        assert_eq!(plan(18, 0, "morning").new_quota, 6);
        assert_eq!(plan(18, 0, "noon").new_quota, 9);
        assert_eq!(plan(18, 0, "evening").new_quota, 18);
    }

    #[test]
    fn 已学的新词从预算里扣除() {
        // 早上学了 6 个，中午应只剩 12 分两场 = 6
        assert_eq!(plan(18, 6, "noon").new_quota, 6);
        // 中午也学完了，晚上剩 6
        assert_eq!(plan(18, 12, "evening").new_quota, 6);
    }

    #[test]
    fn 预算用尽后不再排新词() {
        let p = plan(18, 18, "evening");
        assert_eq!(p.new_quota, 0);
        // 但仍要能复习——缩成空场等于当天白过
        assert_eq!(p.session_words, MIN_SESSION_WORDS);
    }

    #[test]
    fn 超额完成不产生负配额() {
        // 自由练习可以超出预算，此后 remaining 为负
        assert_eq!(plan(18, 25, "morning").new_quota, 0);
    }

    #[test]
    fn 跳过早场时中午自动补上() {
        // 早上没学 → 中午 18/2 = 9，比正常的 6 多，当天仍走得完
        assert_eq!(plan(18, 0, "noon").new_quota, 9);
    }

    #[test]
    fn 自由练习按最后一场处理() {
        // 三时段都做完时自由练习不再给新词，不会让当天翻倍
        assert_eq!(plan(18, 18, "free").new_quota, 0);
        // 预算没用完时把剩余一次给足
        assert_eq!(plan(18, 6, "free").new_quota, 12);
    }

    #[test]
    fn 单场题数受上下限约束() {
        assert_eq!(plan(0, 0, "morning").session_words, MIN_SESSION_WORDS);
        // 每日 60 个、晚场领全部 → 180 题，必须封顶
        assert_eq!(plan(60, 0, "evening").session_words, MAX_SESSION_WORDS);
    }

    #[test]
    fn 缺省预算等价于旧版每场六个() {
        // 旧版 daily_new_words=6 是每场值，三时段共 18。改语义后行为要一致
        assert_eq!(plan(DEFAULT, 0, "morning").new_quota, 6);
        assert_eq!(plan(DEFAULT, 6, "noon").new_quota, 6);
        assert_eq!(plan(DEFAULT, 12, "evening").new_quota, 6);
    }

    #[test]
    fn 今日新词只数首次作答的词() {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();

        let items: Vec<words::WordImport> = ["alpha", "beta"]
            .iter()
            .map(|w| words::WordImport {
                word: (*w).into(),
                phonetic: "/w/".into(),
                pos: "n.".into(),
                meaning: "释义".into(),
                example_1: format!("A {w} appears."),
                example_2: String::new(),
                level: "senior".into(),
                frequency_band: 3,
                frequency_rank: None,
                zone: "grass".into(),
                source_edition: String::new(),
            })
            .collect();
        words::import(&mut conn, &items).unwrap();

        let ids: Vec<i64> = conn
            .prepare("SELECT id FROM words ORDER BY word")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        let today = clock::today();
        let (start, _) = clock::local_day_bounds(&today).unwrap();

        let log = |word_id: i64, at: &str| {
            review_logs::insert(
                &conn,
                &NewReviewLog {
                    word_id,
                    session_id: None,
                    question_type: 1,
                    is_correct: true,
                    reaction_ms: 900,
                    rating: 3,
                    difficulty_before: 5.0,
                    stability_before: 1.0,
                    difficulty_after: 5.0,
                    stability_after: 2.0,
                },
                at,
            )
            .unwrap();
        };

        // alpha 昨天首学、今天复习 → 今天的复习不算新词
        log(ids[0], "2000-01-01T00:00:00Z");
        log(ids[0], &start);
        // beta 今天首学 → 算
        log(ids[1], &start);

        assert_eq!(introduced_on(&conn, &today).unwrap(), 1);
    }

    #[test]
    fn 无作答记录时今日新词为零() {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();
        assert_eq!(introduced_on(&conn, &clock::today()).unwrap(), 0);
    }
}
