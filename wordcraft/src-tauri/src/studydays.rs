//! 学习日。哪几天会弹出训练、赛道按几天计分。
//!
//! ## 为什么需要它
//!
//! 产品原本假设每天都能用电脑——每天三时段、每周 21 个时段。开学后这个前提
//! 不成立：只有周末能用。
//!
//! 连续天数那边已经是对的：`eligible == 0` 判 `Frozen`（决议 S6，「不惩罚
//! 用户未曾获得的机会」），电脑没开的日子既不增也不减。
//!
//! 坏掉的是**赛季赛道**——分母写死 21，周末最多 6 个时段，进度永远停在 29%，
//! 「完美一周」这一档从第一天起就够不着。一个永远达不成的目标不会激励人，
//! 只会让人不再看它。
//!
//! 顺带解决弹窗：上学日在写作业时被单词窗口打断，是纯粹的骚扰。

use rusqlite::Connection;

pub const SETTING_KEY: &str = "study_days";

/// ISO 星期：1 = 周一 … 7 = 周日。
pub type Weekday = u32;

/// 默认每天都学。改成周末要用户自己选——默认值不该替人做减法。
pub const DEFAULT: [Weekday; 7] = [1, 2, 3, 4, 5, 6, 7];

/// 解析 `"6,7"` 这样的设置值。
///
/// 空集合视为非法：一天都不学，应用不如卸载。遇到非法值回落到默认而非
/// 报错——排不出队的后果比多弹一次窗严重得多。
pub fn parse(raw: &str) -> Option<Vec<Weekday>> {
    let mut days: Vec<Weekday> = raw
        .split(',')
        .filter_map(|s| s.trim().parse::<Weekday>().ok())
        .filter(|d| (1..=7).contains(d))
        .collect();
    days.sort_unstable();
    days.dedup();
    if days.is_empty() {
        None
    } else {
        Some(days)
    }
}

pub fn format(days: &[Weekday]) -> String {
    days.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(",")
}

pub fn current(conn: &Connection) -> Result<Vec<Weekday>, String> {
    use crate::db::repo::settings;

    match settings::get(conn, SETTING_KEY)? {
        None => Ok(DEFAULT.to_vec()),
        Some(raw) => match parse(&raw) {
            Some(days) => Ok(days),
            None => {
                log::warn!("settings.{SETTING_KEY} 的值 `{raw}` 无法识别，按每天学习处理");
                Ok(DEFAULT.to_vec())
            }
        },
    }
}

/// 今天是否学习日。
pub fn is_study_day(days: &[Weekday], date: chrono::NaiveDate) -> bool {
    use chrono::Datelike;
    days.contains(&date.weekday().number_from_monday())
}

/// 启动时报告学习日。
pub fn log_current(conn: &Connection) {
    match current(conn) {
        Ok(days) => log::info!("学习日: 周 {}（共 {} 天）", format(&days), days.len()),
        Err(e) => log::warn!("读取学习日失败: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn 默认每天都学() {
        assert_eq!(DEFAULT.len(), 7);
        // 默认替用户砍成周末，会让不受此限的用户莫名少了五天
        assert!(is_study_day(&DEFAULT, d(2026, 8, 26)));
    }

    #[test]
    fn 周末配置只在周六日成立() {
        let weekend = parse("6,7").unwrap();
        assert!(!is_study_day(&weekend, d(2026, 8, 26)), "周三不该是学习日");
        assert!(is_study_day(&weekend, d(2026, 8, 29)), "周六应是");
        assert!(is_study_day(&weekend, d(2026, 8, 30)), "周日应是");
        assert!(!is_study_day(&weekend, d(2026, 8, 31)), "周一不该是");
    }

    #[test]
    fn 解析去重并排序() {
        assert_eq!(parse("7,6,6").unwrap(), vec![6, 7]);
    }

    #[test]
    fn 越界与空值判为非法() {
        // 0 和 8 不是合法星期；全部过滤掉之后集合为空
        assert!(parse("0,8").is_none());
        assert!(parse("").is_none());
        assert!(parse("周六").is_none());
    }

    #[test]
    fn 往返稳定() {
        let days = parse("6,7").unwrap();
        assert_eq!(parse(&format(&days)).unwrap(), days);
    }
}
