//! 时段会话与每日记录。
//!
//! `daily_records.eligible_count` 是 streak 判定的分母（contracts §7.1 / 决议 S6）：
//! 它区分「弹窗出现过但用户没做」与「因全屏检测整个时段静默跳过」。
//! 后者不能计入断签——不能惩罚用户从未获得的机会。

use rusqlite::{Connection, OptionalExtension, Row};
use serde::Serialize;

pub const MAX_POSTPONE: i64 = 3;

#[derive(Debug, Clone, Serialize)]
pub struct Session {
    pub id: i64,
    pub date: String,
    pub session_type: String,
    pub planned_count: i64,
    pub completed_count: i64,
    pub is_completed: bool,
    pub xp_earned: i64,
    pub postpone_count: i64,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DailyRecord {
    pub date: String,
    pub is_paused: bool,
    pub eligible_count: i64,
    pub completed_count: i64,
    pub streak_outcome: String,
}

fn row_to_session(row: &Row) -> rusqlite::Result<Session> {
    Ok(Session {
        id: row.get("id")?,
        date: row.get("date")?,
        session_type: row.get("session_type")?,
        planned_count: row.get("planned_count")?,
        completed_count: row.get("completed_count")?,
        is_completed: row.get::<_, i64>("is_completed")? == 1,
        xp_earned: row.get("xp_earned")?,
        postpone_count: row.get("postpone_count")?,
        started_at: row.get("started_at")?,
        finished_at: row.get("finished_at")?,
    })
}

/// 开始一个时段会话；同日同时段已存在则返回既有记录。
pub fn start(
    conn: &Connection,
    date: &str,
    session_type: &str,
    planned_count: i64,
    now: &str,
) -> Result<Session, String> {
    conn.execute(
        "INSERT INTO sessions (date, session_type, planned_count, started_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(date, session_type) DO NOTHING",
        rusqlite::params![date, session_type, planned_count, now],
    )
    .map_err(|e| format!("创建会话失败: {e}"))?;

    find(conn, date, session_type)?
        .ok_or_else(|| format!("会话 {date}/{session_type} 创建后无法读取"))
}

pub fn find(
    conn: &Connection,
    date: &str,
    session_type: &str,
) -> Result<Option<Session>, String> {
    conn.query_row(
        "SELECT * FROM sessions WHERE date = ?1 AND session_type = ?2",
        [date, session_type],
        row_to_session,
    )
    .optional()
    .map_err(|e| format!("查询会话失败: {e}"))
}

pub fn find_by_id(conn: &Connection, id: i64) -> Result<Option<Session>, String> {
    conn.query_row("SELECT * FROM sessions WHERE id = ?1", [id], row_to_session)
        .optional()
        .map_err(|e| format!("查询会话 {id} 失败: {e}"))
}

pub fn for_date(conn: &Connection, date: &str) -> Result<Vec<Session>, String> {
    let mut stmt = conn
        .prepare("SELECT * FROM sessions WHERE date = ?1 ORDER BY id")
        .map_err(|e| format!("准备会话查询失败: {e}"))?;
    let rows = stmt
        .query_map([date], row_to_session)
        .map_err(|e| format!("查询当日会话失败: {e}"))?;
    rows.collect::<Result<_, _>>()
        .map_err(|e| format!("读取当日会话失败: {e}"))
}

pub fn finish(
    conn: &Connection,
    id: i64,
    completed_count: i64,
    xp_earned: i64,
    now: &str,
) -> Result<(), String> {
    let affected = conn
        .execute(
            "UPDATE sessions
             SET is_completed = 1, completed_count = ?2, xp_earned = ?3, finished_at = ?4
             WHERE id = ?1",
            rusqlite::params![id, completed_count, xp_earned, now],
        )
        .map_err(|e| format!("结束会话 {id} 失败: {e}"))?;

    if affected == 0 {
        return Err(format!("会话 {id} 不存在"));
    }
    Ok(())
}

/// 延后一次，返回剩余可延后次数。
///
/// 已达上限返回 `Err`——spec F1 规定第 4 次不可延后。
pub fn postpone(conn: &Connection, id: i64) -> Result<i64, String> {
    let current: i64 = conn
        .query_row("SELECT postpone_count FROM sessions WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|e| format!("查询延后次数失败: {e}"))?
        .ok_or_else(|| format!("会话 {id} 不存在"))?;

    if current >= MAX_POSTPONE {
        return Err(format!("本时段已延后 {MAX_POSTPONE} 次，不能再延后"));
    }

    conn.execute(
        "UPDATE sessions SET postpone_count = postpone_count + 1 WHERE id = ?1",
        [id],
    )
    .map_err(|e| format!("更新延后次数失败: {e}"))?;

    Ok(MAX_POSTPONE - current - 1)
}

// ─────────────────────────────────────────────
// daily_records
// ─────────────────────────────────────────────

pub fn daily_record(conn: &Connection, date: &str) -> Result<DailyRecord, String> {
    conn.execute(
        "INSERT INTO daily_records (date) VALUES (?1) ON CONFLICT(date) DO NOTHING",
        [date],
    )
    .map_err(|e| format!("创建每日记录失败: {e}"))?;

    conn.query_row(
        "SELECT date, is_paused, eligible_count, completed_count, streak_outcome
         FROM daily_records WHERE date = ?1",
        [date],
        |r| {
            Ok(DailyRecord {
                date: r.get(0)?,
                is_paused: r.get::<_, i64>(1)? == 1,
                eligible_count: r.get(2)?,
                completed_count: r.get(3)?,
                streak_outcome: r.get(4)?,
            })
        },
    )
    .map_err(|e| format!("读取每日记录失败: {e}"))
}

/// 标记某时段「已实际弹出」——streak 判定的分母加一。
///
/// 同一时段重复标记不累加：分母是「有多少个时段给过用户机会」，不是弹窗次数。
pub fn mark_eligible(conn: &Connection, date: &str, session_type: &str) -> Result<(), String> {
    daily_record(conn, date)?;

    // 用 sessions 表的存在性去重：该时段只要建过会话记录，就算给过机会
    let already = find(conn, date, session_type)?.is_some();
    if already {
        return Ok(());
    }

    conn.execute(
        "UPDATE daily_records SET eligible_count = eligible_count + 1 WHERE date = ?1",
        [date],
    )
    .map_err(|e| format!("更新 eligible_count 失败: {e}"))?;
    Ok(())
}

pub fn set_paused(conn: &Connection, date: &str, paused: bool) -> Result<(), String> {
    daily_record(conn, date)?;
    conn.execute(
        "UPDATE daily_records SET is_paused = ?2 WHERE date = ?1",
        rusqlite::params![date, paused as i64],
    )
    .map_err(|e| format!("设置暂停状态失败: {e}"))?;
    Ok(())
}

pub fn set_streak_outcome(conn: &Connection, date: &str, outcome: &str) -> Result<(), String> {
    const VALID: [&str; 6] = [
        "pending", "increment", "perfect", "frozen", "broken", "makeup_used",
    ];
    if !VALID.contains(&outcome) {
        return Err(format!("非法 streak_outcome `{outcome}`"));
    }

    daily_record(conn, date)?;
    let completed = for_date(conn, date)?.iter().filter(|s| s.is_completed).count() as i64;

    conn.execute(
        "UPDATE daily_records SET streak_outcome = ?2, completed_count = ?3 WHERE date = ?1",
        rusqlite::params![date, outcome, completed],
    )
    .map_err(|e| format!("设置 streak 判定结果失败: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;
    use crate::test_support::in_memory_db;

    const D: &str = "2026-08-05";
    const NOW: &str = "2026-08-05T02:00:00Z";

    fn db() -> Connection {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();
        conn
    }

    #[test]
    fn 同日同时段重复开始返回同一会话() {
        let conn = db();
        let a = start(&conn, D, "morning", 5, NOW).unwrap();
        let b = start(&conn, D, "morning", 8, NOW).unwrap();
        assert_eq!(a.id, b.id);
        assert_eq!(b.planned_count, 5, "重复开始不应覆盖原有计划量");
        assert_eq!(for_date(&conn, D).unwrap().len(), 1);
    }

    #[test]
    fn 三个时段互不干扰() {
        let conn = db();
        for t in ["morning", "noon", "evening"] {
            start(&conn, D, t, 5, NOW).unwrap();
        }
        assert_eq!(for_date(&conn, D).unwrap().len(), 3);
        assert_eq!(for_date(&conn, "2026-08-06").unwrap().len(), 0);
    }

    #[test]
    fn 结束会话写入完成标记与_xp() {
        let conn = db();
        let s = start(&conn, D, "morning", 5, NOW).unwrap();
        assert!(!s.is_completed);

        finish(&conn, s.id, 5, 60, NOW).unwrap();
        let done = find_by_id(&conn, s.id).unwrap().unwrap();
        assert!(done.is_completed);
        assert_eq!(done.completed_count, 5);
        assert_eq!(done.xp_earned, 60);
        assert!(done.finished_at.is_some());
    }

    #[test]
    fn 结束不存在的会话报错而非静默成功() {
        let conn = db();
        assert!(finish(&conn, 999, 1, 1, NOW).is_err());
    }

    #[test]
    fn 延后三次后第四次被拒绝() {
        let conn = db();
        let s = start(&conn, D, "morning", 5, NOW).unwrap();

        assert_eq!(postpone(&conn, s.id).unwrap(), 2);
        assert_eq!(postpone(&conn, s.id).unwrap(), 1);
        assert_eq!(postpone(&conn, s.id).unwrap(), 0);

        let err = postpone(&conn, s.id).unwrap_err();
        assert!(err.contains("不能再延后"), "错误消息应说明原因: {err}");
        assert_eq!(
            find_by_id(&conn, s.id).unwrap().unwrap().postpone_count,
            MAX_POSTPONE
        );
    }

    #[test]
    fn 从未弹出的时段不计入_eligible() {
        let conn = db();
        // 只标记 morning 弹出过
        mark_eligible(&conn, D, "morning").unwrap();
        start(&conn, D, "morning", 5, NOW).unwrap();

        let rec = daily_record(&conn, D).unwrap();
        assert_eq!(rec.eligible_count, 1, "只有一个时段给过用户机会");
    }

    #[test]
    fn 同一时段重复标记不重复累加() {
        let conn = db();
        mark_eligible(&conn, D, "morning").unwrap();
        start(&conn, D, "morning", 5, NOW).unwrap();
        mark_eligible(&conn, D, "morning").unwrap();
        mark_eligible(&conn, D, "morning").unwrap();

        assert_eq!(daily_record(&conn, D).unwrap().eligible_count, 1);
    }

    #[test]
    fn 暂停状态可设置与清除() {
        let conn = db();
        assert!(!daily_record(&conn, D).unwrap().is_paused);

        set_paused(&conn, D, true).unwrap();
        assert!(daily_record(&conn, D).unwrap().is_paused);

        set_paused(&conn, D, false).unwrap();
        assert!(!daily_record(&conn, D).unwrap().is_paused);
    }

    #[test]
    fn streak判定结果受控且同步完成数() {
        let conn = db();
        let a = start(&conn, D, "morning", 5, NOW).unwrap();
        let b = start(&conn, D, "noon", 5, NOW).unwrap();
        start(&conn, D, "evening", 5, NOW).unwrap();
        finish(&conn, a.id, 5, 10, NOW).unwrap();
        finish(&conn, b.id, 5, 10, NOW).unwrap();

        set_streak_outcome(&conn, D, "increment").unwrap();
        let rec = daily_record(&conn, D).unwrap();
        assert_eq!(rec.streak_outcome, "increment");
        assert_eq!(rec.completed_count, 2, "完成数应与 sessions 表一致");

        assert!(set_streak_outcome(&conn, D, "maybe").is_err(), "非法值未被拒绝");
    }

    #[test]
    fn 每日记录不存在时自动创建() {
        let conn = db();
        let rec = daily_record(&conn, "2026-12-25").unwrap();
        assert_eq!(rec.eligible_count, 0);
        assert_eq!(rec.streak_outcome, "pending");
    }
}
