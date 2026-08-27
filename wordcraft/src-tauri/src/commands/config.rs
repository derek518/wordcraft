//! 设置读写 command。键契约见 contracts-v1.md §2.1。

use crate::db::{repo::settings, Db};
use rusqlite::Connection;
use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_autostart::ManagerExt;

/// 可写键白名单及其取值约束。
///
/// 白名单而非任意键：settings 是契约的一部分，拼错的键会静默生效为「读不到值→
/// 用默认值」，而用户以为改动已保存。
const WRITABLE: &[(&str, ValueKind)] = &[
    ("onboarding_done", ValueKind::Bool),
    ("placement_stage", ValueKind::IntRange(0, 2)),
    ("session_windows", ValueKind::SessionWindows),
    ("daily_new_words", ValueKind::IntRange(0, 50)),
    ("session_word_count", ValueKind::IntRange(1, 60)),
    ("sound_enabled", ValueKind::Bool),
    ("autostart_enabled", ValueKind::Bool),
    ("tts_provider", ValueKind::OneOf(&["edge", "sapi", "off"])),
    ("daily_pause_date", ValueKind::AnyText),
];

#[derive(Clone, Copy)]
enum ValueKind {
    Bool,
    IntRange(i64, i64),
    OneOf(&'static [&'static str]),
    SessionWindows,
    AnyText,
}

fn validate(key: &str, value: &str) -> Result<(), String> {
    let kind = WRITABLE
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, kind)| *kind)
        .ok_or_else(|| {
            let allowed: Vec<&str> = WRITABLE.iter().map(|(k, _)| *k).collect();
            format!("设置键 `{key}` 不可写，可写键为 {allowed:?}")
        })?;

    match kind {
        ValueKind::Bool => {
            if value != "true" && value != "false" {
                return Err(format!("`{key}` 只接受 true/false，收到 `{value}`"));
            }
        }
        ValueKind::IntRange(lo, hi) => {
            let n: i64 = value
                .parse()
                .map_err(|_| format!("`{key}` 需要整数，收到 `{value}`"))?;
            if !(lo..=hi).contains(&n) {
                return Err(format!("`{key}` 需在 {lo}..{hi}，收到 {n}"));
            }
        }
        ValueKind::OneOf(options) => {
            if !options.contains(&value) {
                return Err(format!("`{key}` 只接受 {options:?}，收到 `{value}`"));
            }
        }
        ValueKind::SessionWindows => validate_session_windows(value)?,
        ValueKind::AnyText => {}
    }
    Ok(())
}

/// 校验 "09:00-11:00,13:00-15:00,19:00-21:00" 形式。
///
/// 时段配置若写坏，调度器会算不出下次弹窗时间而整天不弹——这类故障没有任何
/// 外部症状，用户只会觉得「今天怎么没提醒」。
fn validate_session_windows(value: &str) -> Result<(), String> {
    let windows: Vec<&str> = value.split(',').collect();
    if windows.len() != 3 {
        return Err(format!("需要恰好 3 个时段，收到 {} 个", windows.len()));
    }

    let mut prev_end = 0u32;
    for w in windows {
        let (start, end) = w
            .split_once('-')
            .ok_or_else(|| format!("时段 `{w}` 缺少分隔符 `-`"))?;
        let s = parse_hhmm(start.trim())?;
        let e = parse_hhmm(end.trim())?;
        if s >= e {
            return Err(format!("时段 `{w}` 的开始时间不早于结束时间"));
        }
        if s < prev_end {
            return Err(format!("时段 `{w}` 与前一时段重叠"));
        }
        prev_end = e;
    }
    Ok(())
}

fn parse_hhmm(s: &str) -> Result<u32, String> {
    let (h, m) = s
        .split_once(':')
        .ok_or_else(|| format!("时间 `{s}` 格式应为 HH:MM"))?;
    let h: u32 = h.parse().map_err(|_| format!("小时 `{h}` 非法"))?;
    let m: u32 = m.parse().map_err(|_| format!("分钟 `{m}` 非法"))?;
    if h > 23 || m > 59 {
        return Err(format!("时间 `{s}` 越界"));
    }
    Ok(h * 60 + m)
}

#[tauri::command]
pub fn get_setting(db: State<Db>, key: String) -> Result<Option<String>, String> {
    let conn: std::sync::MutexGuard<'_, Connection> =
        db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))?;
    settings::get(&conn, &key)
}

/// 同步操作系统注册表 / LaunchAgent 与 settings 表。只写表不改系统，重启后开关会骗人。
pub fn apply_autostart<R: Runtime>(app: &AppHandle<R>, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| format!("开启自启失败: {e}"))?;
    } else {
        manager.disable().map_err(|e| format!("关闭自启失败: {e}"))?;
    }
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))?;
    settings::set(&conn, "autostart_enabled", if enabled { "true" } else { "false" })?;
    log::info!("开机自启已{}", if enabled { "开启" } else { "关闭" });
    Ok(())
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    apply_autostart(&app, enabled)
}

#[tauri::command]
pub fn set_setting(app: AppHandle, db: State<Db>, key: String, value: String) -> Result<(), String> {
    validate(&key, &value)?;
    if key == "autostart_enabled" {
        return apply_autostart(&app, value == "true");
    }
    let conn = db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))?;
    settings::set(&conn, &key, &value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 未在白名单的键被拒绝() {
        let err = validate("dayly_new_words", "6").unwrap_err();
        assert!(err.contains("不可写"), "错误消息应指出键不可写: {err}");
        // schema_initialized 是内部标记，不允许前端改写
        assert!(validate("schema_initialized", "false").is_err());
    }

    #[test]
    fn 布尔键只接受_true_false() {
        assert!(validate("sound_enabled", "true").is_ok());
        assert!(validate("sound_enabled", "false").is_ok());
        assert!(validate("sound_enabled", "1").is_err());
        assert!(validate("sound_enabled", "TRUE").is_err());
    }

    #[test]
    fn 整数键强制范围() {
        assert!(validate("daily_new_words", "6").is_ok());
        assert!(validate("daily_new_words", "0").is_ok());
        assert!(validate("daily_new_words", "51").is_err());
        assert!(validate("daily_new_words", "-1").is_err());
        assert!(validate("daily_new_words", "六").is_err());

        assert!(validate("session_word_count", "20").is_ok());
        assert!(validate("session_word_count", "0").is_err());
    }

    #[test]
    fn 枚举键只接受受控值() {
        assert!(validate("tts_provider", "edge").is_ok());
        assert!(validate("tts_provider", "sapi").is_ok());
        assert!(validate("tts_provider", "azure").is_err());
    }

    #[test]
    fn 合法时段配置通过() {
        assert!(validate("session_windows", "09:00-11:00,13:00-15:00,19:00-21:00").is_ok());
        assert!(validate("session_windows", "07:30-09:00,12:00-13:30,20:00-22:00").is_ok());
    }

    #[test]
    fn 非法时段配置被拒绝() {
        // 数量不对
        assert!(validate("session_windows", "09:00-11:00").is_err());
        assert!(validate("session_windows", "09:00-11:00,13:00-15:00,19:00-21:00,22:00-23:00").is_err());
        // 起止颠倒
        assert!(validate("session_windows", "11:00-09:00,13:00-15:00,19:00-21:00").is_err());
        // 重叠
        assert!(validate("session_windows", "09:00-14:00,13:00-15:00,19:00-21:00").is_err());
        // 越界
        assert!(validate("session_windows", "09:00-25:00,13:00-15:00,19:00-21:00").is_err());
        // 格式
        assert!(validate("session_windows", "0900-1100,13:00-15:00,19:00-21:00").is_err());
    }

    #[test]
    fn 时段边界相接不算重叠() {
        assert!(validate("session_windows", "09:00-11:00,11:00-15:00,19:00-21:00").is_ok());
    }
}

/// contracts §3.4：可选的学习范围及各自词数。
///
/// 由数据库现查，前端不写死。词库一更新，选项与数字自动跟上——
/// 写死的计数在本项目已经三次变成谎话。
#[tauri::command]
pub fn get_study_levels(db: State<Db>) -> Result<Vec<crate::scope::LevelOption>, String> {
    let conn = db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))?;
    crate::scope::options(&conn)
}
