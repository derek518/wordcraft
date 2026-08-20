//! 弹窗调度。spec F1：每天三个时段自动弹出训练窗口。
//!
//! 时间推算独立在 `window` 子模块——那是纯逻辑，跨天与跳过规则容易出错，
//! 必须能被穷举测试；本文件只负责与 Tauri 和数据库打交道。

mod window;

pub use window::{next_session, parse_windows, SessionTime};

use crate::db::{clock, repo::sessions, repo::settings, Db};
use chrono::{DateTime, Timelike, Utc};
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// 调度轮询间隔。
///
/// 30 秒而非精确定时：时段窗口有两小时宽度，半分钟误差无关紧要，
/// 而轮询比维护定时器简单得多——系统休眠、时区变更、用户改设置都会让
/// 预设的定时器失效，轮询每轮重新计算，天然自愈。
const TICK: Duration = Duration::from_secs(30);

const DEFAULT_WINDOWS: &str = "09:00-11:00,13:00-15:00,19:00-21:00";

/// 读取配置并计算下一个时段。
fn compute(app: &AppHandle) -> Result<SessionTime, String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))?;

    let raw = settings::get(&conn, "session_windows")?
        .unwrap_or_else(|| DEFAULT_WINDOWS.to_string());
    let windows = parse_windows(&raw)?;

    // 已完成的时段跳过；free 不是定时时段，不参与
    let today = clock::today();
    let completed: Vec<String> = sessions::for_date(&conn, &today)?
        .into_iter()
        .filter(|s| s.is_completed)
        .map(|s| s.session_type)
        .collect();

    next_session(clock::parse_ts(&clock::now())?, &windows, &completed)
}

/// contracts §3.5：下次时段。真实计算，不返回硬编码值。
#[tauri::command]
pub fn get_next_session_time(app: AppHandle) -> Result<SessionTime, String> {
    compute(&app)
}

/// 立即弹出训练窗口。
#[tauri::command]
pub fn trigger_popup_now(app: AppHandle) -> Result<(), String> {
    show_popup(&app)
}

/// 把主窗口显示到前台。
///
/// 失败必须上报：弹不出来正是这个功能的唯一失效模式，
/// 静默返回 Ok 会让「今天怎么没提醒」无从排查。
fn show_popup(app: &AppHandle) -> Result<(), String> {
    let win = app
        .get_webview_window("main")
        .ok_or_else(|| "主窗口不存在".to_string())?;

    win.show().map_err(|e| format!("显示窗口失败: {e}"))?;
    win.unminimize().map_err(|e| format!("取消最小化失败: {e}"))?;
    win.set_focus().map_err(|e| format!("窗口聚焦失败: {e}"))?;
    Ok(())
}

/// 启动调度轮询。
pub fn start_scheduler(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        // 记录已弹过的时段，避免同一窗口内反复弹出。
        // 每次跨到新的一天时清空
        let mut fired: Vec<String> = Vec::new();
        let mut fired_date = clock::today();
        // 周报每小时查一次就够。跟着 30 秒的轮询走会白读几千次配置文件，
        // 而它要等的是「周日 20 点」这种以小时为刻度的条件
        let mut report_hour: Option<u32> = None;

        loop {
            tauri::async_runtime::spawn_blocking(|| std::thread::sleep(TICK))
                .await
                .ok();

            let today = clock::today();
            if today != fired_date {
                fired.clear();
                fired_date = today;
            }

            let hour = chrono::Local::now().hour();
            if report_hour != Some(hour) {
                report_hour = Some(hour);
                check_weekly_report(&app);
            }

            let next = match compute(&app) {
                Ok(n) => n,
                Err(e) => {
                    // 配置写坏时整天不弹，且没有任何外部症状——
                    // 这条日志是唯一线索
                    log::warn!("计算下次时段失败: {e}");
                    continue;
                }
            };

            match apply_postpone(&app, &next.session_type, next.in_window) {
                PostponeDecision::Hold => continue,
                PostponeDecision::Refire => {
                    fired.retain(|t| t != &next.session_type);
                }
                PostponeDecision::Neutral => {}
            }

            if !next.in_window || fired.contains(&next.session_type) {
                continue;
            }

            // 全屏游戏 / 演示中不打扰（spec F1）。
            //
            // 关键在于**不标记 eligible**：这个时段用户从未获得机会，
            // 计入 streak 分母等于惩罚他没做一件根本没被提示的事（决议 S6）。
            // 也不写 fired——等他退出全屏后，本时段仍可正常弹出
            match crate::platform::integration().user_busy_state() {
                Ok(state) if state.should_suppress() => {
                    log::info!("时段 {} 到达但用户处于 {state:?}，本轮跳过", next.session_type);
                    continue;
                }
                Ok(_) => {}
                Err(e) => {
                    // 查不到状态时照常弹：宁可打扰一次，也不要因为检测故障
                    // 让整个提醒功能静默失效
                    log::warn!("查询用户状态失败，按可打扰处理: {e}");
                }
            }

            log::info!("时段 {} 到达，弹出训练窗口", next.session_type);
            if let Err(e) = show_popup(&app) {
                log::warn!("弹出训练窗口失败: {e}");
                continue;
            }

            // 标记「确实弹出过」——这是 streak 判定的分母（决议 S6）。
            // 弹窗成功才标记：失败时不能算用户获得过机会
            if let Err(e) = mark_eligible(&app, &next.session_type) {
                log::warn!("标记时段可用失败: {e}");
            }
            fired.push(next.session_type);
        }
    });
}

/// 周报检查。取不到数据目录时只记日志——周报失效不该影响弹窗调度。
fn check_weekly_report(app: &AppHandle) {
    let dir = match app.path().app_data_dir() {
        Ok(d) => d,
        Err(e) => {
            log::warn!("周报无法定位数据目录: {e}");
            return;
        }
    };
    crate::report::tick(&app.state::<Db>(), &dir);
}

fn mark_eligible(app: &AppHandle, session_type: &str) -> Result<(), String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))?;
    sessions::mark_eligible(&conn, &clock::today(), session_type)
}

#[derive(Debug, PartialEq)]
enum PostponeDecision {
    /// 15 分钟未到，本轮不弹同一时段
    Hold,
    /// 延后到期且仍在窗口，允许再弹一次
    Refire,
    Neutral,
}

fn decide_postpone(
    now: DateTime<Utc>,
    until: Option<DateTime<Utc>>,
    postpone_type: Option<&str>,
    next_type: &str,
    in_window: bool,
) -> PostponeDecision {
    let (Some(until), Some(ptype)) = (until, postpone_type) else {
        return PostponeDecision::Neutral;
    };
    if now < until {
        return if ptype == next_type {
            PostponeDecision::Hold
        } else {
            PostponeDecision::Neutral
        };
    }
    if ptype == next_type && in_window {
        PostponeDecision::Refire
    } else {
        PostponeDecision::Neutral
    }
}

fn apply_postpone(app: &AppHandle, session_type: &str, in_window: bool) -> PostponeDecision {
    let db = app.state::<Db>();
    let Ok(conn) = db.0.lock() else {
        return PostponeDecision::Neutral;
    };
    let until = settings::get(&conn, settings::POSTPONE_UNTIL)
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
        .and_then(|s| clock::parse_ts(&s).ok());
    let ptype = settings::get(&conn, settings::POSTPONE_TYPE)
        .ok()
        .flatten()
        .filter(|s| !s.is_empty());
    let now = clock::parse_ts(&clock::now()).ok();
    let Some(now) = now else {
        return PostponeDecision::Neutral;
    };

    let decision = decide_postpone(now, until, ptype.as_deref(), session_type, in_window);
    if until.is_some_and(|u| now >= u) {
        let _ = settings::set(&conn, settings::POSTPONE_UNTIL, "");
        let _ = settings::set(&conn, settings::POSTPONE_TYPE, "");
    }
    decision
}

#[cfg(test)]
mod postpone_tests {
    use super::*;
    use chrono::Duration;

    fn ts(iso: &str) -> DateTime<Utc> {
        clock::parse_ts(iso).unwrap()
    }

    #[test]
    fn 延后未到期时按住同一时段() {
        let now = ts("2026-08-20T10:00:00Z");
        let until = ts("2026-08-20T10:15:00Z");
        assert_eq!(
            decide_postpone(now, Some(until), Some("morning"), "morning", true),
            PostponeDecision::Hold
        );
    }

    #[test]
    fn 延后到期且仍在窗口则再弹() {
        let now = ts("2026-08-20T10:15:00Z");
        let until = ts("2026-08-20T10:15:00Z");
        assert_eq!(
            decide_postpone(now, Some(until), Some("morning"), "morning", true),
            PostponeDecision::Refire
        );
    }

    #[test]
    fn 延后到期已出窗口则交给下一时段合并() {
        let now = ts("2026-08-20T11:20:00Z");
        let until = now - Duration::minutes(5);
        assert_eq!(
            decide_postpone(now, Some(until), Some("morning"), "noon", true),
            PostponeDecision::Neutral
        );
    }

    #[test]
    fn 没有延后标记时不干预() {
        let now = ts("2026-08-20T10:00:00Z");
        assert_eq!(
            decide_postpone(now, None, None, "morning", true),
            PostponeDecision::Neutral
        );
    }
}
