//! 作答日志。
//!
//! spec §6 要求记录每次作答的完整信号以保证算法可回溯调参——故 FSRS 状态的
//! before/after 都要存。少存一半，日后想复盘「这个间隔是怎么算出来的」就无从查起。

use crate::db::clock;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewReviewLog {
    pub word_id: i64,
    pub session_id: Option<i64>,
    pub question_type: i64,
    pub is_correct: bool,
    pub reaction_ms: i64,
    pub rating: i64,
    pub difficulty_before: f64,
    pub stability_before: f64,
    pub difficulty_after: f64,
    pub stability_after: f64,
}

#[derive(Debug, Default, Serialize)]
pub struct DayStats {
    pub total: i64,
    pub correct: i64,
    pub again: i64,
    pub hard: i64,
    pub good: i64,
    pub easy: i64,
}

pub fn insert(conn: &Connection, log: &NewReviewLog, reviewed_at: &str) -> Result<i64, String> {
    if !(1..=4).contains(&log.rating) {
        return Err(format!("非法 rating {}（FSRS 仅 1-4）", log.rating));
    }
    if !(1..=5).contains(&log.question_type) {
        return Err(format!("非法 question_type {}", log.question_type));
    }
    if log.reaction_ms < 0 {
        return Err("reaction_ms 不能为负".into());
    }

    conn.execute(
        "INSERT INTO review_logs
           (word_id, session_id, question_type, is_correct, reaction_ms, rating,
            difficulty_before, stability_before, difficulty_after, stability_after,
            reviewed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            log.word_id,
            log.session_id,
            log.question_type,
            log.is_correct as i64,
            log.reaction_ms,
            log.rating,
            log.difficulty_before,
            log.stability_before,
            log.difficulty_after,
            log.stability_after,
            reviewed_at,
        ],
    )
    .map_err(|e| format!("写入作答日志失败: {e}"))?;

    Ok(conn.last_insert_rowid())
}

/// 指定本地自然日的作答统计。
pub fn stats_for_day(conn: &Connection, date: &str) -> Result<DayStats, String> {
    let (start, end) = clock::local_day_bounds(date)?;
    stats_in_range(conn, &start, &end)
}

fn stats_in_range(conn: &Connection, start: &str, end: &str) -> Result<DayStats, String> {
    conn.query_row(
        "SELECT
           COUNT(*),
           COALESCE(SUM(is_correct), 0),
           COALESCE(SUM(rating = 1), 0),
           COALESCE(SUM(rating = 2), 0),
           COALESCE(SUM(rating = 3), 0),
           COALESCE(SUM(rating = 4), 0)
         FROM review_logs
         WHERE reviewed_at >= ?1 AND reviewed_at < ?2",
        [start, end],
        |r| {
            Ok(DayStats {
                total: r.get(0)?,
                correct: r.get(1)?,
                again: r.get(2)?,
                hard: r.get(3)?,
                good: r.get(4)?,
                easy: r.get(5)?,
            })
        },
    )
    .map_err(|e| format!("统计作答日志失败: {e}"))
}

pub fn total_count(conn: &Connection) -> Result<i64, String> {
    conn.query_row("SELECT COUNT(*) FROM review_logs", [], |r| r.get(0))
        .map_err(|e| format!("统计累计作答次数失败: {e}"))
}

/// 某词最近一次作答的题型等级。用于「已掌握」判定中的高阶题型条件。
pub fn last_question_type(conn: &Connection, word_id: i64) -> Result<Option<i64>, String> {
    use rusqlite::OptionalExtension;
    conn.query_row(
        "SELECT question_type FROM review_logs
         WHERE word_id = ?1 ORDER BY reviewed_at DESC, id DESC LIMIT 1",
        [word_id],
        |r| r.get(0),
    )
    .optional()
    .map_err(|e| format!("查询词 {word_id} 最近题型失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;
    use crate::db::repo::words::{self, WordImport};
    use crate::test_support::in_memory_db;

    fn db() -> Connection {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();
        words::import(
            &mut conn,
            &[WordImport {
                word: "crystal".into(),
                phonetic: "/k/".into(),
                pos: "n.".into(),
                meaning: "水晶".into(),
                example_1: "A crystal glows.".into(),
                example_2: String::new(),
                level: "junior".into(),
                frequency_band: 1,
                zone: "newbie".into(),
                source_edition: String::new(),
            }],
        )
        .unwrap();
        conn
    }

    fn log(rating: i64, correct: bool) -> NewReviewLog {
        NewReviewLog {
            word_id: 1,
            session_id: None,
            question_type: 1,
            is_correct: correct,
            reaction_ms: 2000,
            rating,
            difficulty_before: 5.0,
            stability_before: 1.0,
            difficulty_after: 4.8,
            stability_after: 3.0,
        }
    }

    #[test]
    fn 写入后可统计且_before_after_均被保留() {
        let conn = db();
        insert(&conn, &log(3, true), &clock::now()).unwrap();

        let (d_before, s_before, d_after, s_after): (f64, f64, f64, f64) = conn
            .query_row(
                "SELECT difficulty_before, stability_before, difficulty_after, stability_after
                 FROM review_logs WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();

        assert_eq!((d_before, s_before), (5.0, 1.0));
        assert_eq!((d_after, s_after), (4.8, 3.0));
        assert_eq!(total_count(&conn).unwrap(), 1);
    }

    #[test]
    fn 非法评级与题型被拒绝() {
        let conn = db();
        assert!(insert(&conn, &log(0, true), &clock::now()).is_err());
        assert!(insert(&conn, &log(5, true), &clock::now()).is_err());

        let mut bad = log(3, true);
        bad.question_type = 6;
        assert!(insert(&conn, &bad, &clock::now()).is_err());

        bad = log(3, true);
        bad.reaction_ms = -1;
        assert!(insert(&conn, &bad, &clock::now()).is_err());
    }

    #[test]
    fn 按评级分类统计() {
        let conn = db();
        let now = clock::now();
        insert(&conn, &log(1, false), &now).unwrap();
        insert(&conn, &log(3, true), &now).unwrap();
        insert(&conn, &log(3, true), &now).unwrap();
        insert(&conn, &log(4, true), &now).unwrap();

        let stats = stats_for_day(&conn, &clock::today()).unwrap();
        assert_eq!(stats.total, 4);
        assert_eq!(stats.correct, 3);
        assert_eq!(stats.again, 1);
        assert_eq!(stats.good, 2);
        assert_eq!(stats.easy, 1);
        assert_eq!(stats.hard, 0);
    }

    #[test]
    fn 跨本地午夜的两条日志分属不同自然日() {
        let conn = db();

        // 取「今天」本地日的 UTC 区间，在其首尾各放一条日志，
        // 再各放一条落在前后相邻日内的日志
        let today = clock::today();
        let (start, end) = clock::local_day_bounds(&today).unwrap();
        let start_dt = clock::parse_ts(&start).unwrap();
        let end_dt = clock::parse_ts(&end).unwrap();

        let just_before = clock::format_ts(start_dt - chrono::Duration::seconds(1));
        let first_moment = start.clone();
        let last_moment = clock::format_ts(end_dt - chrono::Duration::seconds(1));
        let next_day = end.clone();

        insert(&conn, &log(3, true), &just_before).unwrap();
        insert(&conn, &log(3, true), &first_moment).unwrap();
        insert(&conn, &log(3, true), &last_moment).unwrap();
        insert(&conn, &log(3, true), &next_day).unwrap();

        let stats = stats_for_day(&conn, &today).unwrap();
        assert_eq!(
            stats.total, 2,
            "只有落在本地日区间内的两条应被计入今天，实际 {}",
            stats.total
        );
        assert_eq!(total_count(&conn).unwrap(), 4, "四条日志都应已写入");
    }

    #[test]
    fn 无日志的日期统计为零而非报错() {
        let conn = db();
        let stats = stats_for_day(&conn, "2020-01-01").unwrap();
        assert_eq!(stats.total, 0);
        assert_eq!(stats.correct, 0);
    }

    #[test]
    fn 最近题型取最新一条() {
        let conn = db();
        assert_eq!(last_question_type(&conn, 1).unwrap(), None);

        let mut l = log(3, true);
        l.question_type = 1;
        insert(&conn, &l, "2026-08-01T00:00:00Z").unwrap();
        l.question_type = 4;
        insert(&conn, &l, "2026-08-02T00:00:00Z").unwrap();

        assert_eq!(last_question_type(&conn, 1).unwrap(), Some(4));
    }
}
