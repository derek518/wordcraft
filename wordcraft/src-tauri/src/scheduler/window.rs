//! 时段窗口计算。spec F1。
//!
//! 纯逻辑，不碰 Tauri 也不碰数据库——时间推算是这个模块最容易出错的部分
//! （跨天、跨窗口、已完成跳过），必须能被穷举测试。
//!
//! 全部日期运算走 `chrono`（ADR-4）。

use chrono::{DateTime, Duration, Local, NaiveTime, TimeZone, Utc};
use serde::Serialize;

/// 三个时段的固定顺序。与 `session_windows` 配置的三段一一对应。
pub const SESSION_TYPES: [&str; 3] = ["morning", "noon", "evening"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub start: NaiveTime,
    pub end: NaiveTime,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionTime {
    /// 下次时段的开始时刻，本地时间 `HH:MM`
    pub next_session: String,
    pub minutes_until: i64,
    pub session_type: String,
    /// 当前是否正处在某个时段窗口内——此时 `minutes_until` 为 0
    pub in_window: bool,
}

/// 解析 `"09:00-11:00,13:00-15:00,19:00-21:00"`。
///
/// 校验在此处完成而非信任配置：时段写坏会让调度器算不出下次弹窗时间而整天
/// 不弹，这类故障没有任何外部症状，用户只会觉得「今天怎么没提醒」。
pub fn parse_windows(raw: &str) -> Result<Vec<Window>, String> {
    let parts: Vec<&str> = raw.split(',').collect();
    if parts.len() != SESSION_TYPES.len() {
        return Err(format!(
            "需要恰好 {} 个时段，收到 {}",
            SESSION_TYPES.len(),
            parts.len()
        ));
    }

    let mut windows = Vec::with_capacity(parts.len());
    let mut prev_end: Option<NaiveTime> = None;

    for part in parts {
        let (s, e) = part
            .split_once('-')
            .ok_or_else(|| format!("时段 `{part}` 缺少分隔符 `-`"))?;
        let start = parse_hhmm(s.trim())?;
        let end = parse_hhmm(e.trim())?;

        if start >= end {
            return Err(format!("时段 `{part}` 的开始时间不早于结束时间"));
        }
        if let Some(prev) = prev_end {
            if start < prev {
                return Err(format!("时段 `{part}` 与前一时段重叠"));
            }
        }
        prev_end = Some(end);
        windows.push(Window { start, end });
    }
    Ok(windows)
}

fn parse_hhmm(s: &str) -> Result<NaiveTime, String> {
    NaiveTime::parse_from_str(s, "%H:%M").map_err(|_| format!("时间 `{s}` 格式应为 HH:MM"))
}

/// 计算下一个应弹出的时段。
///
/// `completed` 是当日已完成的时段类型。已完成的窗口即便还没结束也要跳过——
/// 用户提前做完了当日任务，不该再被同一时段打扰。
///
/// 全部时段都已完成或错过时，返回次日第一个时段。
pub fn next_session(
    now: DateTime<Utc>,
    windows: &[Window],
    completed: &[String],
) -> Result<SessionTime, String> {
    if windows.len() != SESSION_TYPES.len() {
        return Err(format!("时段数量 {} 不符合预期", windows.len()));
    }

    let local = now.with_timezone(&Local);
    let now_time = local.time();

    for (i, w) in windows.iter().enumerate() {
        let session_type = SESSION_TYPES[i];
        if completed.iter().any(|c| c == session_type) {
            continue;
        }

        // 正处在窗口内：立刻可弹
        if now_time >= w.start && now_time < w.end {
            return Ok(SessionTime {
                next_session: fmt_time(w.start),
                minutes_until: 0,
                session_type: session_type.to_string(),
                in_window: true,
            });
        }

        // 窗口尚未开始：等待
        if now_time < w.start {
            return Ok(SessionTime {
                next_session: fmt_time(w.start),
                minutes_until: minutes_between(now_time, w.start),
                session_type: session_type.to_string(),
                in_window: false,
            });
        }
    }

    // 当日无可用时段 → 次日第一个。
    // 跨天分钟数用 chrono 计算，不手写「1440 - x」之类的算术（ADR-4）
    let tomorrow_start = next_day_at(local, windows[0].start)?;
    let minutes = (tomorrow_start - local).num_minutes().max(0);

    Ok(SessionTime {
        next_session: fmt_time(windows[0].start),
        minutes_until: minutes,
        session_type: SESSION_TYPES[0].to_string(),
        in_window: false,
    })
}

fn fmt_time(t: NaiveTime) -> String {
    t.format("%H:%M").to_string()
}

fn minutes_between(from: NaiveTime, to: NaiveTime) -> i64 {
    (to - from).num_minutes().max(0)
}

/// 次日某时刻的本地时间。
///
/// 用 `and_local_timezone` 而非直接构造：夏令时切换日可能没有某个本地时刻，
/// 或有两个。取最早的合法解，拿不到就报错而非静默用错误时间。
fn next_day_at(local: DateTime<Local>, at: NaiveTime) -> Result<DateTime<Local>, String> {
    let tomorrow = (local + Duration::days(1)).date_naive();
    Local
        .from_local_datetime(&tomorrow.and_time(at))
        .earliest()
        .ok_or_else(|| format!("次日 {at} 在本地时区不存在（夏令时切换）"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    const RAW: &str = "09:00-11:00,13:00-15:00,19:00-21:00";

    fn at(hour: u32, minute: u32) -> DateTime<Utc> {
        let naive = NaiveDate::from_ymd_opt(2026, 8, 6)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap();
        Local
            .from_local_datetime(&naive)
            .earliest()
            .unwrap()
            .with_timezone(&Utc)
    }

    fn windows() -> Vec<Window> {
        parse_windows(RAW).unwrap()
    }

    // ── 解析 ──────────────────────────

    #[test]
    fn 解析合法时段配置() {
        let w = parse_windows(RAW).unwrap();
        assert_eq!(w.len(), 3);
        assert_eq!(w[0].start, NaiveTime::from_hms_opt(9, 0, 0).unwrap());
        assert_eq!(w[2].end, NaiveTime::from_hms_opt(21, 0, 0).unwrap());
    }

    #[test]
    fn 拒绝非法时段配置() {
        // 数量不符
        assert!(parse_windows("09:00-11:00").is_err());
        assert!(parse_windows("09:00-11:00,13:00-15:00,19:00-21:00,22:00-23:00").is_err());
        // 起止颠倒
        assert!(parse_windows("11:00-09:00,13:00-15:00,19:00-21:00").is_err());
        // 重叠
        assert!(parse_windows("09:00-14:00,13:00-15:00,19:00-21:00").is_err());
        // 格式
        assert!(parse_windows("0900-1100,13:00-15:00,19:00-21:00").is_err());
        assert!(parse_windows("09:00-25:00,13:00-15:00,19:00-21:00").is_err());
    }

    #[test]
    fn 时段边界相接不算重叠() {
        assert!(parse_windows("09:00-11:00,11:00-15:00,19:00-21:00").is_ok());
    }

    // ── 窗口内 ──────────────────────────

    #[test]
    fn 处于窗口内时立即可弹() {
        let s = next_session(at(9, 30), &windows(), &[]).unwrap();
        assert_eq!(s.session_type, "morning");
        assert!(s.in_window);
        assert_eq!(s.minutes_until, 0);
    }

    #[test]
    fn 窗口起始时刻即算窗口内() {
        assert!(next_session(at(9, 0), &windows(), &[]).unwrap().in_window);
    }

    #[test]
    fn 窗口结束时刻不再算窗口内() {
        // 11:00 是 morning 的结束，此时应指向 noon
        let s = next_session(at(11, 0), &windows(), &[]).unwrap();
        assert_eq!(s.session_type, "noon");
        assert!(!s.in_window);
    }

    // ── 窗口外 ──────────────────────────

    #[test]
    fn 窗口前等待到开始时刻() {
        let s = next_session(at(8, 30), &windows(), &[]).unwrap();
        assert_eq!(s.session_type, "morning");
        assert_eq!(s.next_session, "09:00");
        assert_eq!(s.minutes_until, 30);
    }

    #[test]
    fn 两窗口之间指向下一个() {
        let s = next_session(at(12, 0), &windows(), &[]).unwrap();
        assert_eq!(s.session_type, "noon");
        assert_eq!(s.minutes_until, 60);
    }

    // ── 已完成跳过 ──────────────────────────

    #[test]
    fn 已完成的时段被跳过() {
        // 用户在 9:30 做完了 morning，不该在同一窗口再被打扰
        let s = next_session(at(9, 30), &windows(), &["morning".into()]).unwrap();
        assert_eq!(s.session_type, "noon");
        assert!(!s.in_window);
    }

    #[test]
    fn 跳过已完成后仍能命中当前窗口() {
        let completed = vec!["morning".to_string()];
        let s = next_session(at(13, 30), &windows(), &completed).unwrap();
        assert_eq!(s.session_type, "noon");
        assert!(s.in_window);
    }

    // ── 跨天 ──────────────────────────

    #[test]
    fn 全部完成后指向次日首个时段() {
        let all: Vec<String> = SESSION_TYPES.iter().map(|s| s.to_string()).collect();
        let s = next_session(at(10, 0), &windows(), &all).unwrap();
        assert_eq!(s.session_type, "morning");
        assert_eq!(s.next_session, "09:00");
        assert!(!s.in_window);
        // 次日 9:00 距今 23 小时
        assert_eq!(s.minutes_until, 23 * 60);
    }

    #[test]
    fn 末班窗口结束后指向次日() {
        let s = next_session(at(22, 0), &windows(), &[]).unwrap();
        assert_eq!(s.session_type, "morning");
        assert_eq!(s.minutes_until, 11 * 60);
    }

    #[test]
    fn 跨天分钟数始终非负() {
        for hour in 0..24 {
            for minute in [0, 30] {
                let s = next_session(at(hour, minute), &windows(), &[]).unwrap();
                assert!(
                    s.minutes_until >= 0,
                    "{hour}:{minute:02} 算出负数 {}",
                    s.minutes_until
                );
                assert!(
                    s.minutes_until <= 24 * 60,
                    "{hour}:{minute:02} 算出超过一天 {}",
                    s.minutes_until
                );
            }
        }
    }

    #[test]
    fn 任意时刻都能算出有效时段() {
        let all: Vec<String> = SESSION_TYPES.iter().map(|s| s.to_string()).collect();
        for hour in 0..24 {
            for completed in [vec![], vec!["morning".to_string()], all.clone()] {
                let s = next_session(at(hour, 0), &windows(), &completed).unwrap();
                assert!(
                    SESSION_TYPES.contains(&s.session_type.as_str()),
                    "{hour}:00 算出未知时段 {}",
                    s.session_type
                );
            }
        }
    }
}
