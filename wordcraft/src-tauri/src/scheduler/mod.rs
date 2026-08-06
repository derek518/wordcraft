//! 弹窗调度。spec F1：每天三个时段自动弹出训练窗口。
//!
//! 时间推算独立在 `window` 子模块——那是纯逻辑，跨天与跳过规则容易出错，
//! 必须能被穷举测试；本文件只负责与 Tauri 和数据库打交道。

mod window;

pub use window::{next_session, parse_windows, SessionTime};

use crate::db::{clock, repo::sessions, repo::settings, Db};
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

        loop {
            tauri::async_runtime::spawn_blocking(|| std::thread::sleep(TICK))
                .await
                .ok();

            let today = clock::today();
            if today != fired_date {
                fired.clear();
                fired_date = today;
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

            if !next.in_window || fired.contains(&next.session_type) {
                continue;
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

fn mark_eligible(app: &AppHandle, session_type: &str) -> Result<(), String> {
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))?;
    sessions::mark_eligible(&conn, &clock::today(), session_type)
}
