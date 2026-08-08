//! 家长周报。spec §4.2 F13。
//!
//! 每周日晚汇总一周学习情况发到家长邮箱。**客户端界面没有任何入口**——
//! spec 的原意是让它对孩子不可见，配置只经由配置文件。
//!
//! 三层分开是刻意的：
//! - `content` 纯数据与排版，可完整测试
//! - `config`  凭据载入与校验，可完整测试
//! - `sender`  真实 SMTP 往返，**本机无法验证**（见 MOCKS.md）
//!
//! 只有最后一层不可测，其余都在测试覆盖内。

pub mod config;
pub mod content;
pub mod sender;

use crate::db::{repo::settings, Db};
use chrono::{Datelike, Duration, Local, NaiveDate, Timelike, Weekday};
use std::path::Path;

/// 周日几点发。晚饭后、睡前，家长大概率会看。
const SEND_HOUR: u32 = 20;

/// 记录最近已发送周次的 settings 键。
///
/// 用 settings 而非新建表：这只是一个标量，不是需要追溯的账本。
const LAST_WEEK_KEY: &str = "last_report_week";

/// 设了这个环境变量就在启动时立刻发一封，用于验证 SMTP 配置。
///
/// 不做成界面按钮——spec 要求周报对孩子完全不可见。环境变量既能让家长
/// 自测配置，又不会在界面上留下任何痕迹。
const TEST_ENV: &str = "WORDCRAFT_REPORT_TEST";

/// 应当汇报的那一周（周一日期）。
///
/// 周日 20:00 之后汇报本周；否则汇报上一整周。这样即便应用整个周日晚都没开，
/// 周一启动时仍会把上周补发出去——错过发送时机不该等于永远不发。
pub fn due_week(now: chrono::DateTime<chrono::Local>) -> NaiveDate {
    let today = now.date_naive();
    let this_week = crate::season::week_start(today);

    if today.weekday() == Weekday::Sun && now.hour() >= SEND_HOUR {
        this_week
    } else {
        this_week - Duration::days(7)
    }
}

/// 本次是否需要发送，以及汇报哪一周。
#[derive(Debug, PartialEq)]
pub enum Decision {
    /// 该周已发过
    AlreadySent,
    /// 首次运行，仅记录基线不发送
    Baseline(NaiveDate),
    Send(NaiveDate),
}

pub fn decide(last_sent: Option<&str>, due: NaiveDate) -> Decision {
    let due_str = due.to_string();
    match last_sent {
        // 首次运行时上一周多半是空的，发一封全零周报只会让家长以为程序坏了。
        // 记下基线，从下一个完整周开始发
        None => Decision::Baseline(due),
        Some(prev) if prev >= due_str.as_str() => Decision::AlreadySent,
        Some(_) => Decision::Send(due),
    }
}

/// 跑一次周报检查。由调度器每轮调用。
///
/// 任何一步失败都只记日志不向上抛：周报失败不该影响主流程，
/// 但也绝不静默——日志是家长排查「怎么没收到」的唯一线索。
pub fn tick(db: &Db, config_dir: &Path) {
    let cfg = match config::load_from(&config::config_path(config_dir)) {
        config::Loaded::Disabled => return,
        config::Loaded::Invalid(e) => {
            log::warn!("周报配置无效，本次跳过: {e}");
            return;
        }
        config::Loaded::Enabled(c) => c,
    };

    let now = Local::now();
    let due = due_week(now);

    let conn = match db.0.lock() {
        Ok(c) => c,
        Err(e) => {
            log::warn!("周报取数据库锁失败: {e}");
            return;
        }
    };

    let last = match settings::get(&conn, LAST_WEEK_KEY) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("读取周报记录失败: {e}");
            return;
        }
    };

    let week = match decide(last.as_deref(), due) {
        Decision::AlreadySent => return,
        Decision::Baseline(w) => {
            if let Err(e) = settings::set(&conn, LAST_WEEK_KEY, &w.to_string()) {
                log::warn!("记录周报基线失败: {e}");
            } else {
                log::info!("周报已启用，基线周 {w}，将从下周开始发送");
            }
            return;
        }
        Decision::Send(w) => w,
    };

    let week_end = week + Duration::days(6);
    let report = match content::build(&conn, &week.to_string(), &week_end.to_string()) {
        Ok(r) => r,
        Err(e) => {
            log::warn!("生成周报内容失败: {e}");
            return;
        }
    };
    drop(conn);

    match deliver(&cfg, &report) {
        Ok(()) => {
            log::info!("周报已发送：{week} 至 {week_end}");
            // 发送成功才记账。失败时保持原值，下一轮会重试——
            // 网络抖动不该让这一周的报告永久丢失
            match db.0.lock() {
                Ok(c) => {
                    if let Err(e) = settings::set(&c, LAST_WEEK_KEY, &week.to_string()) {
                        log::warn!("周报已发出但记账失败，可能重复发送: {e}");
                    }
                }
                Err(e) => log::warn!("周报记账取锁失败: {e}"),
            }
        }
        Err(e) => log::warn!("周报发送失败，下轮重试: {e}"),
    }
}

fn deliver(cfg: &config::SmtpConfig, report: &content::WeeklyReport) -> Result<(), String> {
    let message = sender::compose(cfg, report)?;
    sender::send(cfg, &message)
}

/// 启动时的配置自检与可选试发。
///
/// 配置写错时最糟的情况是「以为配好了，其实一直没发」。启动即校验，
/// 把问题暴露在日志里而不是等到周日。
pub fn startup_check(db: &Db, config_dir: &Path) {
    let path = config::config_path(config_dir);
    match config::load_from(&path) {
        config::Loaded::Disabled => {
            log::info!("周报未启用（未找到 {}）", path.display());
        }
        config::Loaded::Invalid(e) => {
            log::warn!("周报配置有误，功能不会生效: {e}");
        }
        config::Loaded::Enabled(cfg) => {
            log::info!("周报已启用，收件人 {}", cfg.to);
            if std::env::var(TEST_ENV).is_ok() {
                log::info!("{TEST_ENV} 已设置，立即试发一封");
                test_send(db, &cfg);
            }
        }
    }
}

fn test_send(db: &Db, cfg: &config::SmtpConfig) {
    let now = Local::now();
    let week = crate::season::week_start(now.date_naive());
    let week_end = week + Duration::days(6);

    let conn = match db.0.lock() {
        Ok(c) => c,
        Err(e) => {
            log::error!("试发取锁失败: {e}");
            return;
        }
    };
    let report = match content::build(&conn, &week.to_string(), &week_end.to_string()) {
        Ok(r) => r,
        Err(e) => {
            log::error!("试发生成内容失败: {e}");
            return;
        }
    };
    drop(conn);

    match deliver(cfg, &report) {
        Ok(()) => log::info!("试发成功，请检查 {} 的收件箱", cfg.to),
        Err(e) => log::error!("试发失败: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(y: i32, m: u32, d: u32, h: u32) -> chrono::DateTime<chrono::Local> {
        chrono::Local.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap()
    }

    #[test]
    fn 周日晚汇报本周() {
        // 2026-08-09 是周日，本周一为 08-03
        let due = due_week(at(2026, 8, 9, 21));
        assert_eq!(due.to_string(), "2026-08-03");
    }

    #[test]
    fn 周日白天仍汇报上一周() {
        // 一周还没过完，现在发是残缺的
        let due = due_week(at(2026, 8, 9, 10));
        assert_eq!(due.to_string(), "2026-07-27");
    }

    #[test]
    fn 周一启动补发上周() {
        // 应用整个周日晚没开，周一开机应该把上周补上，而不是永远丢掉
        let due = due_week(at(2026, 8, 10, 9));
        assert_eq!(due.to_string(), "2026-08-03");
    }

    #[test]
    fn 周中启动仍指向上一个完整周() {
        let due = due_week(at(2026, 8, 12, 15));
        assert_eq!(due.to_string(), "2026-08-03");
        // 且与周一得到的结论一致——同一周内反复启动不会汇报不同的周
        assert_eq!(due, due_week(at(2026, 8, 10, 9)));
    }

    #[test]
    fn 首次运行只记基线不发信() {
        let due = NaiveDate::from_ymd_opt(2026, 8, 3).unwrap();
        // 首周多半是空的，发全零周报只会让家长以为程序坏了
        assert_eq!(decide(None, due), Decision::Baseline(due));
    }

    #[test]
    fn 同一周不重复发送() {
        let due = NaiveDate::from_ymd_opt(2026, 8, 3).unwrap();
        assert_eq!(decide(Some("2026-08-03"), due), Decision::AlreadySent);
    }

    #[test]
    fn 新的一周触发发送() {
        let due = NaiveDate::from_ymd_opt(2026, 8, 10).unwrap();
        assert_eq!(decide(Some("2026-08-03"), due), Decision::Send(due));
    }

    #[test]
    fn 记录比应发周更新时不倒退发送() {
        // 手工改过系统时间或改过 settings 时，不该倒着补发一堆旧周报
        let due = NaiveDate::from_ymd_opt(2026, 7, 27).unwrap();
        assert_eq!(decide(Some("2026-08-03"), due), Decision::AlreadySent);
    }

    #[test]
    fn 汇报区间恰为七天() {
        let week = NaiveDate::from_ymd_opt(2026, 8, 3).unwrap();
        let end = week + Duration::days(6);
        assert_eq!(end.to_string(), "2026-08-09");
        assert_eq!(week.weekday(), Weekday::Mon);
        assert_eq!(end.weekday(), Weekday::Sun);
    }
}
