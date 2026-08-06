//! 会话生命周期 command。契约见 contracts-v1.md §3.3。
//!
//! 注意 `settle_day` 的 streak 判定（contracts §7.1）不在此处——那需要
//! progression 逻辑，属 T14。本模块只负责会话与每日记录的数据操作。

use crate::db::{clock, repo::sessions, Db};
use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

const VALID_SESSION_TYPES: [&str; 4] = ["morning", "noon", "evening", "free"];

/// 每月「今日暂停」配额，spec F7。
const MONTHLY_PAUSE_QUOTA: i64 = 2;

#[derive(Debug, Serialize)]
pub struct PostponeResult {
    pub remaining: i64,
}

fn lock(db: &Db) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
    db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))
}

fn check_session_type(session_type: &str) -> Result<(), String> {
    if !VALID_SESSION_TYPES.contains(&session_type) {
        return Err(format!(
            "非法的 session_type `{session_type}`，应为 {VALID_SESSION_TYPES:?} 之一"
        ));
    }
    Ok(())
}

#[tauri::command]
pub fn start_session(
    db: State<Db>,
    session_type: String,
    planned_count: i64,
) -> Result<sessions::Session, String> {
    check_session_type(&session_type)?;
    if planned_count <= 0 {
        return Err(format!("planned_count 必须为正数，收到 {planned_count}"));
    }
    let conn = lock(&db)?;
    sessions::start(&conn, &clock::today(), &session_type, planned_count, &clock::now())
}

#[derive(Debug, Serialize)]
pub struct SessionResult {
    pub completed_count: i64,
    pub xp_earned: i64,
    pub total_xp: i64,
    pub level: i64,
}

#[tauri::command]
pub fn finish_session(
    db: State<Db>,
    session_id: i64,
    xp_earned: i64,
) -> Result<SessionResult, String> {
    use crate::db::repo::player_stats;

    if xp_earned < 0 {
        return Err(format!("xp_earned 不能为负，收到 {xp_earned}"));
    }
    let conn = lock(&db)?;

    // 以实际记录的作答数结束会话，而非信任前端传来的数字：
    // 中途退出时前端的计数可能与已落库的 commit_review 笔数不一致
    let session = sessions::find_by_id(&conn, session_id)?
        .ok_or_else(|| format!("会话 {session_id} 不存在"))?;

    if session.is_completed {
        return Err(format!("会话 {session_id} 已结束，不能重复结算"));
    }

    sessions::finish(
        &conn,
        session_id,
        session.completed_count,
        xp_earned,
        &clock::now(),
    )?;

    // XP 必须同时进玩家总账。只写 sessions.xp_earned 的话，用户答完一场看到
    // 结算页有 XP，回到主界面顶栏却仍是 0 —— 会话记了账，玩家没有。
    let (total_xp, level) = player_stats::add_xp(&conn, xp_earned)?;

    Ok(SessionResult {
        completed_count: session.completed_count,
        xp_earned,
        total_xp,
        level,
    })
}

#[tauri::command]
pub fn get_today_sessions(db: State<Db>) -> Result<Vec<sessions::Session>, String> {
    let conn = lock(&db)?;
    sessions::for_date(&conn, &clock::today())
}

/// 延后 15 分钟。达到 spec F1 的 3 次上限后返回 Err。
#[tauri::command]
pub fn postpone_session(db: State<Db>, session_id: i64) -> Result<PostponeResult, String> {
    let conn = lock(&db)?;
    let remaining = sessions::postpone(&conn, session_id)?;
    Ok(PostponeResult { remaining })
}

/// 标记某时段「确实弹出过」。
///
/// 这是 streak 判定的分母（决议 S6）：整个时段都在全屏游戏中而从未弹窗时，
/// 不能算作用户未完成——不能惩罚用户未曾获得的机会。
#[tauri::command]
pub fn mark_session_eligible(db: State<Db>, session_type: String) -> Result<(), String> {
    check_session_type(&session_type)?;
    let conn = lock(&db)?;
    sessions::mark_eligible(&conn, &clock::today(), &session_type)
}

#[tauri::command]
pub fn get_daily_record(db: State<Db>) -> Result<sessions::DailyRecord, String> {
    let conn = lock(&db)?;
    sessions::daily_record(&conn, &clock::today())
}

/// 今日暂停：冻结语义（决议 S8）——当日 streak 不增不减。
///
/// 返回本月剩余次数。配额耗尽返回 Err，不静默失败。
#[tauri::command]
pub fn activate_daily_pause(db: State<Db>) -> Result<i64, String> {
    use crate::db::repo::{player_stats, settings};

    let conn = lock(&db)?;
    let today = clock::today();

    if sessions::daily_record(&conn, &today)?.is_paused {
        return Err("今日已处于暂停状态".to_string());
    }

    let remaining = player_stats::use_pause_quota(&conn, MONTHLY_PAUSE_QUOTA)?;
    sessions::set_paused(&conn, &today, true)?;
    settings::set(&conn, "daily_pause_date", &today)?;

    Ok(remaining)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 会话类型白名单() {
        for t in ["morning", "noon", "evening", "free"] {
            assert!(check_session_type(t).is_ok(), "`{t}` 应被接受");
        }
        assert!(check_session_type("night").is_err());
        assert!(check_session_type("").is_err());
        assert!(check_session_type("Morning").is_err(), "大小写敏感");
    }

    #[test]
    fn 错误消息列出合法取值() {
        let err = check_session_type("night").unwrap_err();
        assert!(err.contains("morning"), "错误消息应列出合法值: {err}");
    }
}
