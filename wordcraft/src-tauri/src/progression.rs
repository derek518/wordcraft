//! Streak 日终结算。契约见 contracts-v1.md §7.1。
//!
//! spec F6 原规则「三时段全完成才计 1 天」与 §1.3 的成功标准「至少完成 2 个」
//! 直接矛盾——用户达到产品自己定义的达标线仍会断签（决议 S1）。此处实现修正后
//! 的规则。
//!
//! **结算必须幂等**：应用每次启动都会补算未结算的日期，重复执行不得重复扣除
//! 补签卡或重复累加 streak。幂等性由 `daily_records.streak_outcome` 保证——
//! 只有 `pending` 的日期会被处理。

use crate::db::{
    clock,
    repo::{player_stats, sessions},
};
use rusqlite::Connection;
use serde::Serialize;

/// 参与 streak 判定的时段。
///
/// `free`（自由探险）不计入——它是额外练习，不是当日任务的一部分。把它算进
/// 分母会让主动多练的用户反而更容易断签。
const STREAK_SESSION_TYPES: [&str; 3] = ["morning", "noon", "evening"];

/// 达标所需完成的时段数（决议 S1）。
///
/// 实际门槛取 `min(REQUIRED_COMPLETED, eligible)`——当天只弹出 1 个时段时，
/// 完成那 1 个即算达标。这与决议 S6「不惩罚用户未曾获得的机会」是同一条原则：
/// S6 已经接受了「机会为 0 不算用户的错」，那么「机会只有 1 个」同理。
const REQUIRED_COMPLETED: usize = 2;

/// 连续冻结天数上限，超过则 streak 归零。
///
/// 冻结日不产生 `daily_records` 行（电脑关着，既没弹窗也没作答），
/// 因此无法靠遍历表来累计——改用「距上次 streak 变动的天数」等价判定。
/// 7 天覆盖周末与短假，又能让「连续天数」这个数字保持意义。
const MAX_FROZEN_DAYS: i64 = 7;

/// 完美日：三个时段全部完成。
const PERFECT_COMPLETED: usize = 3;

/// 完美日奖励的抽卡券数量。
const PERFECT_DAY_TICKETS: i64 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StreakOutcome {
    /// 今日暂停或从未弹出——不增不减
    Frozen,
    /// 达标，streak +1
    Increment,
    /// 三时段全完成，额外奖励
    Perfect,
    /// 未达标且无补签卡，streak 归零
    Broken,
    /// 未达标，自动消耗补签卡保住 streak
    MakeupUsed,
}

impl StreakOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Frozen => "frozen",
            Self::Increment => "increment",
            Self::Perfect => "perfect",
            Self::Broken => "broken",
            Self::MakeupUsed => "makeup_used",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SettleResult {
    pub date: String,
    pub outcome: StreakOutcome,
    pub current_streak: i64,
    pub eligible: usize,
    pub completed: usize,
}

/// 判定当日结果。纯函数，便于穷举契约表格的每一行。
fn decide(is_paused: bool, eligible: usize, completed: usize, has_makeup: bool) -> StreakOutcome {
    if is_paused {
        return StreakOutcome::Frozen;
    }
    // 从未弹出（整天都在全屏应用里，或电脑没开）不计断签——
    // 不能惩罚用户未曾获得的机会（决议 S6）
    if eligible == 0 {
        return StreakOutcome::Frozen;
    }
    if completed >= PERFECT_COMPLETED {
        return StreakOutcome::Perfect;
    }
    // 门槛随当天实际获得的机会数收缩：只弹出 1 个时段时，完成它就算达标
    if completed >= REQUIRED_COMPLETED.min(eligible) {
        return StreakOutcome::Increment;
    }
    if has_makeup {
        return StreakOutcome::MakeupUsed;
    }
    StreakOutcome::Broken
}

/// 结算指定日期。已结算的日期直接返回既有结果，不重复计算。
pub fn settle(conn: &Connection, date: &str) -> Result<SettleResult, String> {
    let record = sessions::daily_record(conn, date)?;
    let stats = player_stats::get(conn)?;

    if record.streak_outcome != "pending" {
        return Ok(SettleResult {
            date: date.to_string(),
            outcome: parse_outcome(&record.streak_outcome)?,
            current_streak: stats.current_streak,
            eligible: record.eligible_count as usize,
            completed: record.completed_count as usize,
        });
    }

    let day_sessions = sessions::for_date(conn, date)?;
    let completed = day_sessions
        .iter()
        .filter(|s| s.is_completed && STREAK_SESSION_TYPES.contains(&s.session_type.as_str()))
        .count();

    // eligible 取「标记过弹出」与「实际启动过」的较大值：用户主动点开传送门
    // 同样是获得了机会，即便调度器没来得及标记
    let started = day_sessions
        .iter()
        .filter(|s| STREAK_SESSION_TYPES.contains(&s.session_type.as_str()))
        .count();
    let eligible = (record.eligible_count as usize).max(started);

    let outcome = decide(
        record.is_paused,
        eligible,
        completed,
        stats.makeup_cards > 0,
    );

    let current_streak = apply_outcome(conn, outcome, stats.current_streak, date)?;
    sessions::set_streak_outcome(
        conn,
        date,
        outcome.as_str(),
        eligible as i64,
        completed as i64,
    )?;

    Ok(SettleResult {
        date: date.to_string(),
        outcome,
        current_streak,
        eligible,
        completed,
    })
}

fn parse_outcome(raw: &str) -> Result<StreakOutcome, String> {
    match raw {
        "frozen" => Ok(StreakOutcome::Frozen),
        "increment" => Ok(StreakOutcome::Increment),
        "perfect" => Ok(StreakOutcome::Perfect),
        "broken" => Ok(StreakOutcome::Broken),
        "makeup_used" => Ok(StreakOutcome::MakeupUsed),
        other => Err(format!("未知的 streak_outcome `{other}`，schema 与代码已脱节")),
    }
}

/// 把判定结果落到 player_stats，返回新的 current_streak。
///
/// 注意 `player_stats::set_streak` 返回的是 `best_streak` 而非新的当前值——
/// 此处显式返回计算结果，不依赖它的返回值。
fn apply_outcome(
    conn: &Connection,
    outcome: StreakOutcome,
    current: i64,
    date: &str,
) -> Result<i64, String> {
    match outcome {
        // 冻结不写 last_streak_date：连续冻结的天数正是靠这个间隔来累计的
        StreakOutcome::Frozen => Ok(current),

        StreakOutcome::Increment => {
            player_stats::set_streak(conn, current + 1, date)?;
            Ok(current + 1)
        }

        StreakOutcome::Perfect => {
            player_stats::set_streak(conn, current + 1, date)?;
            player_stats::add_draw_tickets(conn, PERFECT_DAY_TICKETS)?;
            Ok(current + 1)
        }

        StreakOutcome::MakeupUsed => {
            // consume 返回 false 说明卡已被消耗，此时应视为断签而非静默保留
            if player_stats::consume_makeup_card(conn)? {
                // 补签保住的是既有天数，但仍要更新 last_streak_date——
                // 否则这一天会被当作冻结日计入连续冻结天数
                player_stats::set_streak(conn, current, date)?;
                Ok(current)
            } else {
                player_stats::set_streak(conn, 0, date)?;
                Ok(0)
            }
        }

        StreakOutcome::Broken => {
            player_stats::set_streak(conn, 0, date)?;
            Ok(0)
        }
    }
}

/// 长期未使用导致的 streak 失效检查。
///
/// 关机的日子既没有 `sessions` 也没有 `daily_records` 行，`settle_pending_days`
/// 遍历不到它们，所以连续冻结天数无法靠查表累计。改用等价判定：`last_streak_date`
/// 只在 streak 实际变动时更新，冻结日不写——因此「距上次变动的天数」就是
/// 连续冻结天数。
///
/// 返回是否发生了失效。
pub fn expire_stale_streak(conn: &Connection, today: &str) -> Result<bool, String> {
    let stats = player_stats::get(conn)?;
    if stats.current_streak == 0 {
        return Ok(false);
    }

    let Some(last) = stats.last_streak_date else {
        return Ok(false);
    };

    if clock::days_between(&last, today)? > MAX_FROZEN_DAYS {
        player_stats::set_streak(conn, 0, today)?;
        log::info!(
            "距上次学习已超过 {MAX_FROZEN_DAYS} 天（{last} → {today}），连续天数重置"
        );
        return Ok(true);
    }
    Ok(false)
}

/// 每月 1 日发放补签卡（决议 S4：MVP 阶段不依赖赛道积分）。
pub fn grant_monthly_makeup(conn: &Connection) -> Result<bool, String> {
    let month = clock::today()[..7].to_string(); // YYYY-MM
    player_stats::grant_monthly_if_due(conn, &month)
}

/// 补算所有未结算的历史日期。
///
/// 应用启动时调用。用户关机数日后重开，中间的日期从未被结算——若不补算，
/// `daily_records` 会留下一串 `pending`，streak 停在关机前的数字，
/// 之后每天的判定都建立在过期的基数上。
///
/// 返回按日期升序排列的结算结果。
pub fn settle_pending_days(conn: &Connection, today: &str) -> Result<Vec<SettleResult>, String> {
    let pending = pending_dates_before(conn, today)?;
    let mut results = Vec::with_capacity(pending.len());

    // 必须按时间顺序结算：streak 是累积量，乱序会算错
    for date in pending {
        results.push(settle(conn, &date)?);
    }
    Ok(results)
}

/// 查出 `today` 之前所有待结算的日期，按升序排列。
///
/// 待结算日期有两个来源，缺一不可：
/// - `daily_records` 中 `streak_outcome = 'pending'` 的——调度器标记过弹出，或用户暂停过
/// - 只有 `sessions` 行、还没有 `daily_records` 行的——用户直接点开传送门训练，
///   而 `sessions::start` 并不创建 `daily_records` 行
///
/// 只查前者会让第二类日期永远结算不到，streak 停在原地不动。
fn pending_dates_before(conn: &Connection, today: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT date FROM (
                 SELECT date FROM daily_records WHERE streak_outcome = 'pending'
                 UNION
                 SELECT s.date FROM sessions s
                 LEFT JOIN daily_records d ON d.date = s.date
                 WHERE d.date IS NULL
             )
             WHERE date < ?1
             ORDER BY date ASC",
        )
        .map_err(|e| format!("准备待结算日期查询失败: {e}"))?;

    let rows = stmt
        .query_map([today], |r| r.get::<_, String>(0))
        .map_err(|e| format!("查询待结算日期失败: {e}"))?;

    rows.collect::<Result<_, _>>()
        .map_err(|e| format!("读取待结算日期失败: {e}"))
}

/// 启动时的日终结算。
///
/// 顺序不可调换：
/// 1. 先发月度补签卡——否则 1 号断签时手里没卡，本该保住的 streak 会丢
/// 2. 再补算历史日期——按时间顺序推进 streak，同时刷新 `last_streak_date`
/// 3. 最后做失效检查——它依赖前一步刷新后的 `last_streak_date`
pub fn run_daily_rollover(db: &crate::db::Db) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))?;
    let today = clock::today();

    if grant_monthly_makeup(&conn)? {
        log::info!("已发放本月补签卡");
    }

    let settled = settle_pending_days(&conn, &today)?;
    if !settled.is_empty() {
        log::info!(
            "补算了 {} 个未结算日期，当前连续 {} 天",
            settled.len(),
            settled.last().map(|r| r.current_streak).unwrap_or(0)
        );
    }

    expire_stale_streak(&conn, &today)?;
    Ok(())
}

/// contracts §3.3：结算所有未结算的历史日期。
///
/// 除启动时自动执行外，前端也需要能主动触发——应用整夜开着跨过午夜时，
/// 前一天从未被结算，用户第二天早上看到的仍是过期的连续天数。
#[tauri::command]
pub fn settle_day(db: tauri::State<crate::db::Db>) -> Result<Vec<SettleResult>, String> {
    let conn = db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))?;
    let today = clock::today();
    let settled = settle_pending_days(&conn, &today)?;
    expire_stale_streak(&conn, &today)?;
    Ok(settled)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── 判定表穷举（contracts §7.1）──────────────────────────

    #[test]
    fn 今日暂停一律冻结() {
        // 暂停优先于其他所有条件
        assert_eq!(decide(true, 3, 0, false), StreakOutcome::Frozen);
        assert_eq!(decide(true, 3, 3, true), StreakOutcome::Frozen);
        assert_eq!(decide(true, 0, 0, false), StreakOutcome::Frozen);
    }

    #[test]
    fn 从未弹出时冻结而非断签() {
        // 决议 S6：整天全屏游戏 / 电脑没开，用户没有获得机会
        assert_eq!(decide(false, 0, 0, false), StreakOutcome::Frozen);
        assert_eq!(decide(false, 0, 0, true), StreakOutcome::Frozen, "冻结不应消耗补签卡");
    }

    #[test]
    fn 完成两个时段即达标() {
        // 决议 S1：原 spec 要求三个全完成，与 §1.3 的成功标准矛盾
        assert_eq!(decide(false, 3, 2, false), StreakOutcome::Increment);
        assert_eq!(decide(false, 2, 2, false), StreakOutcome::Increment);
    }

    #[test]
    fn 完成三个时段为完美日() {
        assert_eq!(decide(false, 3, 3, false), StreakOutcome::Perfect);
    }

    #[test]
    fn 未达标时优先消耗补签卡() {
        assert_eq!(decide(false, 3, 1, true), StreakOutcome::MakeupUsed);
        assert_eq!(decide(false, 3, 0, true), StreakOutcome::MakeupUsed);
    }

    #[test]
    fn 未达标且无补签卡则断签() {
        assert_eq!(decide(false, 3, 1, false), StreakOutcome::Broken);
        assert_eq!(decide(false, 3, 0, false), StreakOutcome::Broken);
        assert_eq!(decide(false, 1, 0, false), StreakOutcome::Broken);
    }

    #[test]
    fn 机会不足两个时完成全部即达标() {
        // 门槛随机会数收缩：只弹出 1 个时段、完成了它 → 达标。
        // 与决议 S6 同源——不惩罚用户未曾获得的机会
        assert_eq!(decide(false, 1, 1, false), StreakOutcome::Increment);
        // 弹出 2 个只完成 1 个仍算未达标
        assert_eq!(decide(false, 2, 1, false), StreakOutcome::Broken);
    }

    #[test]
    fn 补签卡不会替代本可达标的日子() {
        // 有补签卡但已达标时不应消耗
        assert_eq!(decide(false, 3, 2, true), StreakOutcome::Increment);
        assert_eq!(decide(false, 1, 1, true), StreakOutcome::Increment);
    }

    // ── 落库行为 ──────────────────────────

    use crate::db::{migrations, repo::sessions};
    use crate::test_support::in_memory_db;

    fn db() -> Connection {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();
        conn
    }

    /// 造一个已完成的时段。
    fn complete_session(conn: &Connection, date: &str, session_type: &str) {
        let s = sessions::start(conn, date, session_type, 20, &clock::now()).unwrap();
        sessions::finish(conn, s.id, 20, 100, &clock::now()).unwrap();
    }

    #[test]
    fn 达标日累加连续天数() {
        let conn = db();
        let date = "2026-08-06";
        complete_session(&conn, date, "morning");
        complete_session(&conn, date, "noon");

        let r = settle(&conn, date).unwrap();
        assert_eq!(r.outcome, StreakOutcome::Increment);
        assert_eq!(r.current_streak, 1, "返回值应是当前连续天数而非历史最佳");
        assert_eq!(player_stats::get(&conn).unwrap().current_streak, 1);
    }

    #[test]
    fn 完美日额外发放抽卡券() {
        let conn = db();
        let date = "2026-08-06";
        for t in ["morning", "noon", "evening"] {
            complete_session(&conn, date, t);
        }
        let before = player_stats::get(&conn).unwrap().draw_tickets;

        let r = settle(&conn, date).unwrap();
        assert_eq!(r.outcome, StreakOutcome::Perfect);
        assert_eq!(
            player_stats::get(&conn).unwrap().draw_tickets,
            before + PERFECT_DAY_TICKETS
        );
    }

    #[test]
    fn 重复结算不重复累加() {
        let conn = db();
        let date = "2026-08-06";
        complete_session(&conn, date, "morning");
        complete_session(&conn, date, "noon");

        settle(&conn, date).unwrap();
        let first = player_stats::get(&conn).unwrap().current_streak;

        // 应用每次启动都会补算，幂等性是硬要求
        for _ in 0..5 {
            let again = settle(&conn, date).unwrap();
            assert_eq!(again.outcome, StreakOutcome::Increment);
        }
        assert_eq!(
            player_stats::get(&conn).unwrap().current_streak,
            first,
            "重复结算导致连续天数虚增"
        );
    }

    #[test]
    fn 重复结算不重复消耗补签卡() {
        let conn = db();
        let date = "2026-08-06";
        sessions::mark_eligible(&conn, date, "morning").unwrap();
        sessions::mark_eligible(&conn, date, "noon").unwrap();
        player_stats::set_streak(&conn, 5, "2026-08-05").unwrap();

        // 新用户初始 0 张，补签卡靠每月发放（决议 S4）
        player_stats::grant_monthly_if_due(&conn, "2026-08").unwrap();
        let before = player_stats::get(&conn).unwrap().makeup_cards;
        assert!(before > 0, "月度发放后应有补签卡");

        settle(&conn, date).unwrap();
        let after_first = player_stats::get(&conn).unwrap().makeup_cards;
        assert_eq!(after_first, before - 1);

        settle(&conn, date).unwrap();
        assert_eq!(
            player_stats::get(&conn).unwrap().makeup_cards,
            after_first,
            "重复结算重复扣卡"
        );
    }

    #[test]
    fn 自由探险不计入达标判定() {
        let conn = db();
        let date = "2026-08-06";
        complete_session(&conn, date, "morning");
        complete_session(&conn, date, "free");
        complete_session(&conn, date, "free"); // UNIQUE 约束会让第二次失败，忽略

        let r = settle(&conn, date).unwrap();
        assert_eq!(r.completed, 1, "free 不应计入完成数");
    }

    #[test]
    fn 补算按日期升序执行() {
        let conn = db();
        for date in ["2026-08-01", "2026-08-02", "2026-08-03"] {
            complete_session(&conn, date, "morning");
            complete_session(&conn, date, "noon");
        }

        let results = settle_pending_days(&conn, "2026-08-04").unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].date, "2026-08-01");
        assert_eq!(results[2].date, "2026-08-03");
        // 顺序错了会算出不同的连续天数
        assert_eq!(results[2].current_streak, 3);
    }

    #[test]
    fn 补算不包含今天() {
        let conn = db();
        complete_session(&conn, "2026-08-06", "morning");

        let results = settle_pending_days(&conn, "2026-08-06").unwrap();
        assert!(results.is_empty(), "当天尚未过完，不应提前结算");
    }

    #[test]
    fn 补算覆盖只有会话记录的日期() {
        let conn = db();
        // 用户直接点开传送门训练，调度器从未标记弹出：
        // 这天只有 sessions 行，没有 daily_records 行
        complete_session(&conn, "2026-08-01", "morning");
        complete_session(&conn, "2026-08-01", "noon");

        let results = settle_pending_days(&conn, "2026-08-02").unwrap();
        assert_eq!(
            results.len(),
            1,
            "只有 sessions 记录的日期也必须结算，否则 streak 永远不涨"
        );
        assert_eq!(results[0].outcome, StreakOutcome::Increment);
    }

    // ── 长期未使用失效 ──────────────────────────

    #[test]
    fn 连续冻结超过上限则归零() {
        let conn = db();
        player_stats::set_streak(&conn, 12, "2026-08-01").unwrap();

        // 恰好 7 天，未超过
        assert!(!expire_stale_streak(&conn, "2026-08-08").unwrap());
        assert_eq!(player_stats::get(&conn).unwrap().current_streak, 12);

        // 第 8 天，超过上限
        assert!(expire_stale_streak(&conn, "2026-08-09").unwrap());
        assert_eq!(player_stats::get(&conn).unwrap().current_streak, 0);
    }

    #[test]
    fn 失效检查保留历史最佳() {
        let conn = db();
        player_stats::set_streak(&conn, 30, "2026-08-01").unwrap();
        expire_stale_streak(&conn, "2026-09-01").unwrap();

        let stats = player_stats::get(&conn).unwrap();
        assert_eq!(stats.current_streak, 0);
        assert_eq!(stats.best_streak, 30, "历史最佳不应被清除");
    }

    #[test]
    fn 连续天数为零时无需失效检查() {
        let conn = db();
        assert!(!expire_stale_streak(&conn, "2026-12-31").unwrap());
    }

    #[test]
    fn 从未学习过的用户不触发失效() {
        let conn = db();
        // last_streak_date 为 NULL
        assert!(!expire_stale_streak(&conn, "2026-08-06").unwrap());
    }

    #[test]
    fn 跨月的天数差计算正确() {
        let conn = db();
        player_stats::set_streak(&conn, 5, "2026-01-28").unwrap();
        // 跨月且 2026 非闰年：1/28 → 2/5 是 8 天
        assert!(expire_stale_streak(&conn, "2026-02-05").unwrap());
    }
}
