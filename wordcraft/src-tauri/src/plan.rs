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
//! ## 预算按「负担」扣，不按「词数」扣
//!
//! 一个词答对了、而且答得快，对孩子来说本来就算不上新词——它几乎不产生
//! 后续复习。把它和一个完全不会的词同等扣预算，等于因为「今天运气好、
//! 遇到的词都会」而提前收工。
//!
//! 所以预算按首答评级加权扣除（见 `cost_of`）。全部答成 Easy 的一天，
//! 能拿到设定值两倍的词量；一个词都不会的一天，就是设定值本身。
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

/// 词数相对预算的硬上限。
///
/// 已经会的词少算负担，但不能无限放大：全部 Easy 时按 0.25 折算意味着
/// 四倍词量，单场会长到坐不住——目标用户有 ADHD 特征。两倍是个能坐得住
/// 的上限，而且水平真的高的话 θ 会跟着涨，前沿上移，词自然就不再是 Easy。
const MAX_RAW_MULTIPLIER: i64 = 2;

/// 首答评级对应的预算消耗。
///
/// 实测（真实库 165 词 / 409 次作答，按首答评级分组）：
///
/// ```text
/// 评级        平均总作答   首次间隔    相对负担
/// 1 Again        5.85      1.0 天      1.00
/// 2 Hard         3.30      4.5 天      0.56
/// 3 Good         3.11      4.2 天      0.53
/// 4 Easy         1.52     10.4 天      0.26
/// ```
///
/// 差异来自 FSRS 的初始间隔，是算法机制而非个人特征——换个人作答，
/// 给定评级之后的负担比例不变。Hard 与 Good 实测几乎没有区别，合成一档。
fn cost_of(rating: i64) -> f64 {
    match rating {
        4 => 0.25,     // Easy：基本已经会了
        2 | 3 => 0.55, // Hard / Good
        _ => 1.0,      // Again：真的是生词，要反复练
    }
}

/// 每个新词平均占用的题数（首次学习 + 当天的后续复习）。
///
/// FSRS 的学习阶段会让新词在同一天内再出现一到两次，加上首次学习，
/// 三倍是实测的稳态比例。这个系数把「新词预算」翻译成「单场题数」。
const WORDS_PER_NEW: i64 = 3;

/// 今天已经用掉多少。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DayUsage {
    /// 加权消耗，预算按这个扣
    pub consumed: f64,
    /// 实际词数，受 `MAX_RAW_MULTIPLIER` 约束
    pub raw: i64,
}

/// 一次会话的学习量安排。全部由每日预算推算，无第二个设置项。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SessionPlan {
    /// 每日新词预算（设置值）
    pub daily_budget: i64,
    /// 今天已经用掉多少
    pub used: DayUsage,
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

/// 今天首见的词用掉了多少预算。
///
/// 判据是该词的**最早一条**作答日志落在今天，而不是 `word_states` 的存在性——
/// 后者曾被摸底预建上千条，用它计数会把整个词库算成今天学的。
///
/// 用 `MIN(id)` 而非 `MIN(reviewed_at)` 定位首条：时间戳只到秒，同一个词
/// 在一场里被重排两次可能撞上同一秒，按时间戳取会取出两行。
pub fn usage_on(conn: &Connection, date: &str) -> Result<DayUsage, String> {
    let (start, end) = clock::local_day_bounds(date)?;
    let mut stmt = conn
        .prepare(
            "SELECT r.rating, COUNT(*) FROM review_logs r
              WHERE r.id IN (SELECT MIN(id) FROM review_logs GROUP BY word_id)
                AND r.reviewed_at >= ?1 AND r.reviewed_at < ?2
              GROUP BY r.rating",
        )
        .map_err(|e| format!("准备今日新词统计失败: {e}"))?;
    let rows = stmt
        .query_map([&start, &end], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
        .map_err(|e| format!("统计今日新词失败: {e}"))?;

    let mut usage = DayUsage::default();
    for row in rows {
        let (rating, count) = row.map_err(|e| format!("读取今日新词统计失败: {e}"))?;
        usage.consumed += cost_of(rating) * count as f64;
        usage.raw += count;
    }
    Ok(usage)
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
    let used = usage_on(conn, date)?;
    Ok(compute(daily_budget, used, session_type))
}

/// 一天最多能给到多少个词。
///
/// 全部答成 Easy 时按 0.25 折算本可给四倍，硬上限压到两倍——见
/// `MAX_RAW_MULTIPLIER`。界面用它展示区间，不在前端乘一遍。
pub fn max_daily_words(daily_budget: i64) -> i64 {
    daily_budget.max(0) * MAX_RAW_MULTIPLIER
}

/// 纯函数部分，便于直接测边界。
pub fn compute(daily_budget: i64, used: DayUsage, session_type: &str) -> SessionPlan {
    // 预算按负担扣：今天遇到的词要是都会，就还剩很多额度
    let by_budget = (daily_budget as f64 - used.consumed).max(0.0).ceil() as i64;
    // 但词数有硬上限，否则全 Easy 的一天会排出坐不住的长场
    let by_count = (daily_budget * MAX_RAW_MULTIPLIER - used.raw).max(0);
    let remaining = by_budget.min(by_count);

    let left = sessions_left(session_type);
    // 向上取整分摊。不用 div_ceil：该方法在 Rust 1.93 仍是 unstable（int_roundings）
    let new_quota = (remaining + left - 1) / left;
    let session_words = (new_quota * WORDS_PER_NEW).clamp(MIN_SESSION_WORDS, MAX_SESSION_WORDS);

    SessionPlan {
        daily_budget,
        used,
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

    /// `done` 是「今天已学的生词数」，按最重的一档（Again）折算——
    /// 老测试都在测「已学多少就扣多少」这条逻辑
    fn plan(budget: i64, done: i64, st: &str) -> SessionPlan {
        compute(budget, DayUsage { consumed: done as f64, raw: done }, st)
    }

    /// 今天遇到的词都答成 Easy
    fn plan_easy(budget: i64, done: i64, st: &str) -> SessionPlan {
        compute(budget, DayUsage { consumed: cost_of(4) * done as f64, raw: done }, st)
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
    fn 已经会的词少扣预算() {
        // 早上遇到 12 个词全答成 Easy：按词数算预算该用光了（18 的三分之二），
        // 但它们几乎不产生复习负担，中午还该有额度
        let easy = plan_easy(18, 12, "noon");
        let hard = plan(18, 12, "noon");
        assert!(
            easy.new_quota > hard.new_quota,
            "全会的 12 个词与全不会的 12 个词扣掉了同样的预算（{} vs {}）",
            easy.new_quota,
            hard.new_quota
        );
    }

    #[test]
    fn 全都答对时一天能给到两倍词量() {
        // 一天从头开始，全部 Easy——预算按 0.25 折算本可给四倍，
        // 硬上限压到两倍，否则单场会长到坐不住
        let mut used = DayUsage::default();
        let mut total = 0;
        for _ in 0..10 {
            let p = compute(18, used, "evening");
            if p.new_quota == 0 {
                break;
            }
            total += p.new_quota;
            used.consumed += cost_of(4) * p.new_quota as f64;
            used.raw += p.new_quota;
        }
        assert_eq!(total, 18 * 2, "全会的一天应给到设定值的两倍，实得 {total}");
    }

    #[test]
    fn 全都答错时一天只给设定值() {
        let mut used = DayUsage::default();
        let mut total = 0;
        for _ in 0..10 {
            let p = compute(18, used, "evening");
            if p.new_quota == 0 {
                break;
            }
            total += p.new_quota;
            used.consumed += cost_of(1) * p.new_quota as f64;
            used.raw += p.new_quota;
        }
        // 一个都不会的一天就是设定值本身——那才是真正的学习负担上限
        assert_eq!(total, 18, "全不会的一天不该超出设定值，实得 {total}");
    }

    #[test]
    fn 负担权重按实测排序() {
        // 实测（真实库 165 词）：首答 Again 平均要答 5.85 次，Easy 只要 1.52 次。
        // 权重反了的话，越会的词扣得越多
        assert!(cost_of(1) > cost_of(3));
        assert!(cost_of(3) > cost_of(4));
        // Hard 与 Good 实测几乎无差别（3.30 / 3.11），合成一档
        assert_eq!(cost_of(2), cost_of(3));
        // Again 是基准
        assert_eq!(cost_of(1), 1.0);
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
    fn 今日用量只数首次作答的词() {
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

        assert_eq!(usage_on(&conn, &today).unwrap().raw, 1);
    }

    #[test]
    fn 今日用量按首答评级加权() {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();

        let items: Vec<words::WordImport> = ["alpha", "beta", "gamma"]
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
                frequency_rank: Some(3000),
                zone: "grass".into(),
                source_edition: String::new(),
            })
            .collect();
        words::import(&mut conn, &items).unwrap();

        let today = clock::today();
        let (start, _) = clock::local_day_bounds(&today).unwrap();
        let log = |word_id: i64, rating: i64, at: &str| {
            review_logs::insert(
                &conn,
                &NewReviewLog {
                    word_id,
                    session_id: None,
                    question_type: 1,
                    is_correct: rating > 1,
                    reaction_ms: 900,
                    rating,
                    difficulty_before: 5.0,
                    stability_before: 1.0,
                    difficulty_after: 5.0,
                    stability_after: 2.0,
                },
                at,
            )
            .unwrap();
        };

        log(1, 4, &start); // Easy
        log(2, 3, &start); // Good
        log(3, 1, &start); // Again
        // 同一个词的第二次作答是复习，不该再扣预算
        log(1, 3, &start);

        let u = usage_on(&conn, &today).unwrap();
        assert_eq!(u.raw, 3, "只数首见，复习不算");
        let expect = cost_of(4) + cost_of(3) + cost_of(1);
        assert!(
            (u.consumed - expect).abs() < 1e-9,
            "加权消耗应为 {expect}，得到 {}",
            u.consumed
        );
    }

    #[test]
    fn 无作答记录时今日用量为零() {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();
        assert_eq!(usage_on(&conn, &clock::today()).unwrap(), DayUsage::default());
    }
}
