//! 赛季赛道。spec §4.2 F11。
//!
//! 幽灵车是**上周的自己**而非其他用户——spec 明确「无社交对比」。
//! 目标用户对同伴压力敏感，与自己赛跑既有推力又不会变成负担。

mod scoring;

pub use scoring::{
    can_afford, points_for_total, ratio_of, week_start, SESSIONS_PER_DAY, REDEEM_DRAW_TICKET, REDEEM_MAKEUP_CARD,
};

use crate::db::{clock, repo::player_stats, Db};
use chrono::NaiveDate;
use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

/// 参与赛道计数的时段。`free` 不计——它是额外练习，
/// 计入会让主动多练的人轻易刷满赛道
const TRACK_SESSION_TYPES: [&str; 3] = ["morning", "noon", "evening"];

#[derive(Debug, Serialize)]
pub struct SeasonState {
    /// 本周周一
    pub week_start: String,
    /// 本周周日。与 week_start 一同给出，免得前端各自再算一遍日历
    pub week_end: String,
    pub sessions_done: i64,
    pub sessions_total: i64,
    /// 赛车位置 0.0–1.0
    pub progress: f64,
    /// 幽灵车位置：上周同一天的进度
    pub ghost_progress: f64,
    pub ghost_sessions: i64,
    /// 本周若现在结算能得多少分
    pub projected_points: i64,
    pub track_points: i64,
    /// 计分参数。前端要在里程碑上标出「到这里能拿多少分」，
    /// 而那个数字只能有一个来源——写死在前端必然与这里漂移
    pub points_per_session: i64,
    pub perfect_bonus: i64,
}

#[derive(Debug, Serialize)]
pub struct SettleOutcome {
    pub settled_weeks: Vec<String>,
    pub points_gained: i64,
    pub track_points: i64,
}

fn lock(db: &Db) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
    db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))
}

fn parse_date(s: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| format!("日期 `{s}` 解析失败: {e}"))
}

/// 某周内已完成的时段数。
///
/// 从 sessions 实时聚合而非另存一份计数——两个真相来源迟早对不上。
fn sessions_in_week(conn: &Connection, start: NaiveDate, days: i64) -> Result<i64, String> {
    let end = start + chrono::Duration::days(days);
    let placeholders = TRACK_SESSION_TYPES
        .iter()
        .map(|t| format!("'{t}'"))
        .collect::<Vec<_>>()
        .join(",");

    conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM sessions
             WHERE is_completed = 1
               AND session_type IN ({placeholders})
               AND date >= ?1 AND date < ?2"
        ),
        rusqlite::params![start.to_string(), end.to_string()],
        |r| r.get(0),
    )
    .map_err(|e| format!("统计周内会话失败: {e}"))
}

fn state_of(conn: &Connection, today: NaiveDate) -> Result<SeasonState, String> {
    let start = week_start(today);
    // 已过天数 +1：周一当天算第 1 天
    let elapsed = (today - start).num_days() + 1;

    let done = sessions_in_week(conn, start, 7)?;

    // 幽灵车取上周**同期**而非整周：周三跟上周整周比毫无意义，
    // 永远显示自己落后
    let last_week = start - chrono::Duration::days(7);
    let ghost = sessions_in_week(conn, last_week, elapsed)?;

    // 分母按学习日数算，不是写死的 21。只有周末能用时，21 这个目标
    // 从第一天起就够不着——够不着的目标不激励人，只会让人不再看它
    let study_days = crate::studydays::current(conn)?.len() as i64;
    let total = study_days * SESSIONS_PER_DAY;

    Ok(SeasonState {
        week_start: start.to_string(),
        week_end: (start + chrono::Duration::days(6)).to_string(),
        sessions_done: done,
        sessions_total: total,
        progress: ratio_of(done, total),
        ghost_progress: ratio_of(ghost, total),
        ghost_sessions: ghost,
        projected_points: points_for_total(done, total),
        track_points: player_stats::get(conn)?.track_points,
        points_per_session: scoring::POINTS_PER_SESSION,
        perfect_bonus: scoring::PERFECT_WEEK_BONUS,
    })
}

/// 结算所有已过去且未结算的周。
///
/// 幂等靠 `season_settlements` 的主键——启动时调用，重复执行不重复发分。
pub fn settle_past_weeks(conn: &mut Connection, today: NaiveDate) -> Result<SettleOutcome, String> {
    let current = week_start(today);

    // 找出有会话记录但尚未结算的周。只回溯到最早一条会话，
    // 不做无限往前推
    let earliest: Option<String> = conn
        .query_row("SELECT MIN(date) FROM sessions", [], |r| r.get(0))
        .ok()
        .flatten();

    let Some(earliest) = earliest else {
        return Ok(SettleOutcome {
            settled_weeks: Vec::new(),
            points_gained: 0,
            track_points: player_stats::get(conn)?.track_points,
        });
    };

    let week_total = crate::studydays::current(conn)?.len() as i64 * SESSIONS_PER_DAY;
    let mut week = week_start(parse_date(&earliest)?);
    let mut pending: Vec<(String, i64, i64)> = Vec::new();

    while week < current {
        let key = week.to_string();
        let already: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM season_settlements WHERE week_start = ?1",
                [&key],
                |r| r.get(0),
            )
            .map_err(|e| format!("查询结算记录失败: {e}"))?;

        if already == 0 {
            let done = sessions_in_week(conn, week, 7)?;
            // 结算同样按学习日口径。用写死的 21 判，只在周末学的用户
            // 每一周都拿不到完美奖励——那是在惩罚他上学
            pending.push((key, done, points_for_total(done, week_total)));
        }
        week += chrono::Duration::days(7);
    }

    if pending.is_empty() {
        return Ok(SettleOutcome {
            settled_weeks: Vec::new(),
            points_gained: 0,
            track_points: player_stats::get(conn)?.track_points,
        });
    }

    let now = clock::now();
    // 整批一个事务：结算记录与积分必须同生共死，否则记了账却没发分，
    // 而主键会让那些分永远补不回来
    let tx = conn
        .transaction()
        .map_err(|e| format!("开启结算事务失败: {e}"))?;

    let mut gained = 0i64;
    let mut weeks = Vec::new();
    for (key, done, points) in pending {
        tx.execute(
            "INSERT INTO season_settlements
             (week_start, sessions_done, points_earned, settled_at)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![key, done, points, now],
        )
        .map_err(|e| format!("写入结算记录失败: {e}"))?;
        gained += points;
        weeks.push(key);
    }

    if gained > 0 {
        tx.execute(
            "UPDATE player_stats SET track_points = track_points + ?1 WHERE id = 1",
            [gained],
        )
        .map_err(|e| format!("发放赛道积分失败: {e}"))?;
    }

    tx.commit().map_err(|e| format!("提交结算事务失败: {e}"))?;

    if !weeks.is_empty() {
        log::info!("结算 {} 个赛季周，获得 {gained} 赛道积分", weeks.len());
    }

    Ok(SettleOutcome {
        settled_weeks: weeks,
        points_gained: gained,
        track_points: player_stats::get(conn)?.track_points,
    })
}

/// 启动时结算。失败只记 warn——积分是奖励，拿不到远不如打不开应用严重。
pub fn settle_on_startup(db: &Db) -> Result<(), String> {
    let mut conn = db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))?;
    let today = parse_date(&clock::today())?;
    settle_past_weeks(&mut conn, today)?;
    Ok(())
}

// ─────────────────────────────────────────────
// Commands
// ─────────────────────────────────────────────

#[tauri::command]
pub fn get_season(db: State<Db>) -> Result<SeasonState, String> {
    let conn = lock(&db)?;
    state_of(&conn, parse_date(&clock::today())?)
}

#[derive(Debug, Serialize)]
pub struct RedeemOutcome {
    pub track_points: i64,
    pub draw_tickets: i64,
    pub makeup_cards: i64,
}

/// 兑换。积分不足返回 Err，不静默失败。
#[tauri::command]
pub fn redeem_points(db: State<Db>, item: String) -> Result<RedeemOutcome, String> {
    let (cost, column) = match item.as_str() {
        "draw_ticket" => (REDEEM_DRAW_TICKET, "draw_tickets"),
        "makeup_card" => (REDEEM_MAKEUP_CARD, "makeup_cards"),
        other => return Err(format!("未知兑换项 `{other}`")),
    };

    let conn = lock(&db)?;
    let stats = player_stats::get(&conn)?;

    if !can_afford(stats.track_points, cost) {
        return Err(format!(
            "赛道积分不足：需要 {cost}，当前 {}",
            stats.track_points
        ));
    }

    // 扣分与发物必须同步。SQLite 的单条语句是原子的，但这是两条，
    // 故显式加事务
    conn.execute("BEGIN", [])
        .map_err(|e| format!("开启兑换事务失败: {e}"))?;

    let result = (|| -> Result<(), String> {
        conn.execute(
            "UPDATE player_stats SET track_points = track_points - ?1 WHERE id = 1",
            [cost],
        )
        .map_err(|e| format!("扣除积分失败: {e}"))?;
        conn.execute(
            &format!("UPDATE player_stats SET {column} = {column} + 1 WHERE id = 1"),
            [],
        )
        .map_err(|e| format!("发放兑换物失败: {e}"))?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            conn.execute("COMMIT", [])
                .map_err(|e| format!("提交兑换失败: {e}"))?;
        }
        Err(e) => {
            let _ = conn.execute("ROLLBACK", []);
            return Err(e);
        }
    }

    let after = player_stats::get(&conn)?;
    Ok(RedeemOutcome {
        track_points: after.track_points,
        draw_tickets: after.draw_tickets,
        makeup_cards: after.makeup_cards,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{migrations, repo::sessions};
    use crate::test_support::in_memory_db;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn db() -> Connection {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();
        conn
    }

    /// 在指定日期完成若干时段。
    fn complete(conn: &Connection, date: &str, types: &[&str]) {
        for t in types {
            let s = sessions::start(conn, date, t, 20, &clock::now()).unwrap();
            sessions::finish(conn, s.id, 20, 100, &clock::now()).unwrap();
        }
    }

    #[test]
    fn 赛道分母跟随学习日设置() {
        let conn = db();
        crate::db::repo::settings::set(&conn, crate::studydays::SETTING_KEY, "6,7").unwrap();

        let s = state_of(&conn, d("2026-08-05")).unwrap();
        // 只在周末学：2 天 × 3 时段 = 6，而不是写死的 21。
        // 分母不跟着走，「完美一周」从第一天起就够不着
        assert_eq!(s.sessions_total, 6);
    }

    #[test]
    fn 周末练满即算完美周() {
        let conn = db();
        crate::db::repo::settings::set(&conn, crate::studydays::SETTING_KEY, "6,7").unwrap();
        // 2026-08-08 周六、08-09 周日
        complete(&conn, "2026-08-08", &["morning", "noon", "evening"]);
        complete(&conn, "2026-08-09", &["morning", "noon", "evening"]);

        let s = state_of(&conn, d("2026-08-09")).unwrap();
        assert_eq!(s.progress, 1.0, "六个时段就是他的一个不落");
        assert_eq!(
            s.projected_points,
            6 * scoring::POINTS_PER_SESSION + scoring::PERFECT_WEEK_BONUS,
            "拿不到完美奖励等于惩罚他上学"
        );
    }

    #[test]
    fn 本周进度按已完成时段计() {
        let conn = db();
        // 2026-08-03 周一
        complete(&conn, "2026-08-03", &["morning", "noon"]);
        complete(&conn, "2026-08-04", &["morning"]);

        let s = state_of(&conn, d("2026-08-05")).unwrap();
        assert_eq!(s.sessions_done, 3);
        assert_eq!(s.week_start, "2026-08-03");
        assert!((s.progress - 3.0 / 21.0).abs() < 1e-9);
    }

    #[test]
    fn 自由探险不计入赛道() {
        let conn = db();
        complete(&conn, "2026-08-03", &["morning", "free"]);

        // free 计入会让主动多练的人轻易刷满赛道
        let s = state_of(&conn, d("2026-08-03")).unwrap();
        assert_eq!(s.sessions_done, 1);
    }

    #[test]
    fn 幽灵车取上周同期而非整周() {
        let conn = db();
        // 上周（7-27 周一）完成 6 个时段，分布在周一到周三
        complete(&conn, "2026-07-27", &["morning", "noon", "evening"]);
        complete(&conn, "2026-07-28", &["morning", "noon", "evening"]);
        complete(&conn, "2026-07-30", &["morning"]);

        // 本周才到周二，幽灵车应只算上周前两天的 6 个
        let s = state_of(&conn, d("2026-08-04")).unwrap();
        assert_eq!(s.ghost_sessions, 6, "周二跟上周整周比，永远显示自己落后");
    }

    #[test]
    fn 未过完的周不结算() {
        let mut conn = db();
        complete(&conn, "2026-08-03", &["morning"]);

        // 本周尚未结束
        let out = settle_past_weeks(&mut conn, d("2026-08-05")).unwrap();
        assert!(out.settled_weeks.is_empty());
        assert_eq!(out.points_gained, 0);
    }

    #[test]
    fn 过去的周结算并发放积分() {
        let mut conn = db();
        complete(&conn, "2026-07-27", &["morning", "noon", "evening"]);

        let out = settle_past_weeks(&mut conn, d("2026-08-05")).unwrap();
        assert_eq!(out.settled_weeks, vec!["2026-07-27"]);
        assert_eq!(out.points_gained, 30);
        assert_eq!(player_stats::get(&conn).unwrap().track_points, 30);
    }

    #[test]
    fn 重复结算不重复发分() {
        let mut conn = db();
        complete(&conn, "2026-07-27", &["morning", "noon"]);
        settle_past_weeks(&mut conn, d("2026-08-05")).unwrap();
        let first = player_stats::get(&conn).unwrap().track_points;

        // 每次启动都会调用
        for _ in 0..5 {
            settle_past_weeks(&mut conn, d("2026-08-05")).unwrap();
        }
        assert_eq!(player_stats::get(&conn).unwrap().track_points, first);
    }

    #[test]
    fn 跨多周离线后逐周补算() {
        let mut conn = db();
        complete(&conn, "2026-07-13", &["morning"]);
        complete(&conn, "2026-07-20", &["morning", "noon"]);
        complete(&conn, "2026-07-27", &["morning", "noon", "evening"]);

        let out = settle_past_weeks(&mut conn, d("2026-08-05")).unwrap();
        assert_eq!(out.settled_weeks.len(), 3, "离线数周后应逐周补算");
        assert_eq!(out.points_gained, 10 + 20 + 30);
    }

    #[test]
    fn 无会话记录时结算不报错() {
        let mut conn = db();
        let out = settle_past_weeks(&mut conn, d("2026-08-05")).unwrap();
        assert!(out.settled_weeks.is_empty());
    }

    #[test]
    fn 完美周结算含额外奖励() {
        let mut conn = db();
        for day in 27..=31 {
            complete(&conn, &format!("2026-07-{day}"), &["morning", "noon", "evening"]);
        }
        complete(&conn, "2026-08-01", &["morning", "noon", "evening"]);
        complete(&conn, "2026-08-02", &["morning", "noon", "evening"]);

        let out = settle_past_weeks(&mut conn, d("2026-08-05")).unwrap();
        assert_eq!(
            out.points_gained,
            21 * scoring::POINTS_PER_SESSION + scoring::PERFECT_WEEK_BONUS
        );
    }
}
