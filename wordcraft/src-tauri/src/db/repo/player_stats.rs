//! 玩家总状态（单行表）。公式见 contracts-v1.md §7。

use rusqlite::Connection;
use serde::Serialize;

/// MVP 阶段每月自动发放的补签卡数量（决议 S4）。
/// spec 原设计依赖 P1 的赛道积分兑换，会导致 MVP 期间断签无任何补救途径。
pub const MONTHLY_MAKEUP_CARDS: i64 = 2;
pub const MAX_LEVEL: i64 = 100;

#[derive(Debug, Clone, Serialize)]
pub struct PlayerStats {
    pub total_xp: i64,
    pub level: i64,
    pub current_streak: i64,
    pub best_streak: i64,
    pub last_streak_date: Option<String>,
    pub vocab_estimate: i64,
    pub makeup_cards: i64,
    pub pause_used_month: i64,
    pub draw_tickets: i64,
    /// 赛道积分（migration 007）。spec F11：断签不清，只清 streak
    pub track_points: i64,
    pub last_grant_month: Option<String>,
}

/// 等级公式：`floor(sqrt(total_xp / 50)) + 1`，上限 100。
pub fn level_for_xp(total_xp: i64) -> i64 {
    let level = ((total_xp.max(0) as f64) / 50.0).sqrt().floor() as i64 + 1;
    level.min(MAX_LEVEL)
}

pub fn get(conn: &Connection) -> Result<PlayerStats, String> {
    conn.query_row("SELECT * FROM player_stats WHERE id = 1", [], |r| {
        Ok(PlayerStats {
            total_xp: r.get("total_xp")?,
            level: r.get("level")?,
            current_streak: r.get("current_streak")?,
            best_streak: r.get("best_streak")?,
            last_streak_date: r.get("last_streak_date")?,
            vocab_estimate: r.get("vocab_estimate")?,
            makeup_cards: r.get("makeup_cards")?,
            pause_used_month: r.get("pause_used_month")?,
            draw_tickets: r.get("draw_tickets")?,
            track_points: r.get("track_points")?,
            last_grant_month: r.get("last_grant_month")?,
        })
    })
    .map_err(|e| format!("读取玩家状态失败: {e}"))
}

/// 增加 XP 并同步等级。返回新的 (total_xp, level)。
pub fn add_xp(conn: &Connection, delta: i64) -> Result<(i64, i64), String> {
    if delta < 0 {
        return Err("XP 增量不能为负".into());
    }
    let total = get(conn)?.total_xp + delta;
    let level = level_for_xp(total);

    conn.execute(
        "UPDATE player_stats SET total_xp = ?1, level = ?2 WHERE id = 1",
        [total, level],
    )
    .map_err(|e| format!("更新 XP 失败: {e}"))?;

    Ok((total, level))
}

/// 设置连续天数，并同步历史最佳。
pub fn set_streak(conn: &Connection, streak: i64, date: &str) -> Result<i64, String> {
    if streak < 0 {
        return Err("streak 不能为负".into());
    }
    let best = get(conn)?.best_streak.max(streak);

    conn.execute(
        "UPDATE player_stats
         SET current_streak = ?1, best_streak = ?2, last_streak_date = ?3
         WHERE id = 1",
        rusqlite::params![streak, best, date],
    )
    .map_err(|e| format!("更新 streak 失败: {e}"))?;

    Ok(best)
}

/// 消耗一张补签卡；无卡可用返回 `Ok(false)`。
pub fn consume_makeup_card(conn: &Connection) -> Result<bool, String> {
    let affected = conn
        .execute(
            "UPDATE player_stats SET makeup_cards = makeup_cards - 1
             WHERE id = 1 AND makeup_cards > 0",
            [],
        )
        .map_err(|e| format!("消耗补签卡失败: {e}"))?;
    Ok(affected > 0)
}

/// 月度发放补签卡并重置暂停配额；同一月份只发一次。
///
/// 幂等由 `last_grant_month` 保证——否则每次启动都会补一次卡。
pub fn grant_monthly_if_due(conn: &Connection, month: &str) -> Result<bool, String> {
    if get(conn)?.last_grant_month.as_deref() == Some(month) {
        return Ok(false);
    }

    conn.execute(
        "UPDATE player_stats
         SET makeup_cards = ?1, pause_used_month = 0, last_grant_month = ?2
         WHERE id = 1",
        rusqlite::params![MONTHLY_MAKEUP_CARDS, month],
    )
    .map_err(|e| format!("发放月度补签卡失败: {e}"))?;

    log::info!("已发放 {month} 月度补签卡 {MONTHLY_MAKEUP_CARDS} 张，暂停配额已重置");
    Ok(true)
}

pub fn add_draw_tickets(conn: &Connection, delta: i64) -> Result<i64, String> {
    let total = (get(conn)?.draw_tickets + delta).max(0);
    conn.execute(
        "UPDATE player_stats SET draw_tickets = ?1 WHERE id = 1",
        [total],
    )
    .map_err(|e| format!("更新抽卡券失败: {e}"))?;
    Ok(total)
}

pub fn use_pause_quota(conn: &Connection, monthly_limit: i64) -> Result<i64, String> {
    let used = get(conn)?.pause_used_month;
    if used >= monthly_limit {
        return Err(format!("本月「今日暂停」配额已用完（{monthly_limit} 次）"));
    }
    conn.execute(
        "UPDATE player_stats SET pause_used_month = pause_used_month + 1 WHERE id = 1",
        [],
    )
    .map_err(|e| format!("更新暂停配额失败: {e}"))?;
    Ok(monthly_limit - used - 1)
}

pub fn set_vocab_estimate(conn: &Connection, estimate: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE player_stats SET vocab_estimate = ?1 WHERE id = 1",
        [estimate.max(0)],
    )
    .map_err(|e| format!("写入词汇量估计失败: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{clock, migrations};
    use crate::test_support::in_memory_db;

    fn db() -> Connection {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();
        conn
    }

    #[test]
    fn 初始状态为零() {
        let s = get(&db()).unwrap();
        assert_eq!(s.total_xp, 0);
        assert_eq!(s.level, 1);
        assert_eq!(s.current_streak, 0);
        assert_eq!(s.makeup_cards, 0);
    }

    #[test]
    fn 等级公式在关键点取值正确() {
        assert_eq!(level_for_xp(0), 1);
        assert_eq!(level_for_xp(49), 1);
        assert_eq!(level_for_xp(50), 2);
        assert_eq!(level_for_xp(200), 3);
        assert_eq!(level_for_xp(450), 4);
        assert_eq!(level_for_xp(500_000), MAX_LEVEL, "等级应封顶在 100");
        assert_eq!(level_for_xp(-100), 1, "负 XP 不应产生非法等级");
    }

    #[test]
    fn 累加_xp_同步等级() {
        let conn = db();
        assert_eq!(add_xp(&conn, 30).unwrap(), (30, 1));
        assert_eq!(add_xp(&conn, 20).unwrap(), (50, 2));
        assert_eq!(add_xp(&conn, 150).unwrap(), (200, 3));
        assert_eq!(get(&conn).unwrap().level, 3);
        assert!(add_xp(&conn, -1).is_err(), "负增量应被拒绝");
    }

    #[test]
    fn 最佳_streak_只增不减() {
        let conn = db();
        set_streak(&conn, 7, "2026-08-05").unwrap();
        assert_eq!(get(&conn).unwrap().best_streak, 7);

        set_streak(&conn, 0, "2026-08-06").unwrap();
        let s = get(&conn).unwrap();
        assert_eq!(s.current_streak, 0, "当前 streak 应被清零");
        assert_eq!(s.best_streak, 7, "历史最佳不应被清零");
    }

    #[test]
    fn 月度发放幂等且跨月重新发放() {
        let conn = db();
        assert!(grant_monthly_if_due(&conn, "2026-08").unwrap());
        assert_eq!(get(&conn).unwrap().makeup_cards, MONTHLY_MAKEUP_CARDS);

        assert!(!grant_monthly_if_due(&conn, "2026-08").unwrap(), "同月不应重复发放");
        consume_makeup_card(&conn).unwrap();
        assert_eq!(get(&conn).unwrap().makeup_cards, MONTHLY_MAKEUP_CARDS - 1);
        assert!(!grant_monthly_if_due(&conn, "2026-08").unwrap());

        assert!(grant_monthly_if_due(&conn, "2026-09").unwrap(), "跨月应重新发放");
        assert_eq!(get(&conn).unwrap().makeup_cards, MONTHLY_MAKEUP_CARDS);
    }

    #[test]
    fn 补签卡耗尽后不再消耗() {
        let conn = db();
        grant_monthly_if_due(&conn, "2026-08").unwrap();
        for _ in 0..MONTHLY_MAKEUP_CARDS {
            assert!(consume_makeup_card(&conn).unwrap());
        }
        assert!(!consume_makeup_card(&conn).unwrap(), "无卡时应返回 false");
        assert_eq!(get(&conn).unwrap().makeup_cards, 0, "不应出现负数");
    }

    #[test]
    fn 暂停配额用尽后报错() {
        let conn = db();
        assert_eq!(use_pause_quota(&conn, 2).unwrap(), 1);
        assert_eq!(use_pause_quota(&conn, 2).unwrap(), 0);
        assert!(use_pause_quota(&conn, 2).is_err(), "超配额应报错");
    }

    #[test]
    fn 跨月发放重置暂停配额() {
        let conn = db();
        grant_monthly_if_due(&conn, "2026-08").unwrap();
        use_pause_quota(&conn, 2).unwrap();
        use_pause_quota(&conn, 2).unwrap();

        grant_monthly_if_due(&conn, "2026-09").unwrap();
        assert_eq!(get(&conn).unwrap().pause_used_month, 0);
        assert!(use_pause_quota(&conn, 2).is_ok(), "新月份应恢复配额");
    }

    #[test]
    fn 抽卡券增减不出现负数() {
        let conn = db();
        assert_eq!(add_draw_tickets(&conn, 3).unwrap(), 3);
        assert_eq!(add_draw_tickets(&conn, -1).unwrap(), 2);
        assert_eq!(add_draw_tickets(&conn, -10).unwrap(), 0, "不应变为负数");
    }

    #[test]
    fn 词汇量估计可写入() {
        let conn = db();
        set_vocab_estimate(&conn, 1200).unwrap();
        assert_eq!(get(&conn).unwrap().vocab_estimate, 1200);
        set_vocab_estimate(&conn, -5).unwrap();
        assert_eq!(get(&conn).unwrap().vocab_estimate, 0);
    }

    #[test]
    fn 当前月份格式可直接用于发放判定() {
        let conn = db();
        let month = clock::current_month();
        assert!(grant_monthly_if_due(&conn, &month).unwrap());
        assert!(!grant_monthly_if_due(&conn, &month).unwrap());
    }
}
