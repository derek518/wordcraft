// 与 db::repo 同理：本模块经 repo 层被间接使用，而 repo 层要到 T10 才完成接线，
// 在此之前从 main 出发不可达。T10 结束时连同 repo/mod.rs 的同类 allow 一并移除。
#![allow(dead_code)]

//! 时间处理。所有日期运算的唯一入口。
//!
//! ADR-4：禁止手写日历运算，一律走 chrono。
//! ADR-5：存储时间戳一律 UTC ISO8601；「今天」的归属按**本地时区**计算。
//!
//! 两者的区别是本模块存在的理由：`review_logs.reviewed_at` 存 UTC，但
//! 「今天答了多少词」「今天三个时段完成了几个」必须按用户所在时区的自然日
//! 划分——否则在 UTC+8，每天早上 8 点之前的作答都会被算进前一天。

use chrono::{DateTime, Datelike, Days, Local, NaiveDate, TimeZone, Utc};

/// 存储用的时间戳格式。
const TS_FORMAT: &str = "%Y-%m-%dT%H:%M:%SZ";
/// 存储用的日期格式。
const DATE_FORMAT: &str = "%Y-%m-%d";

/// 当前时刻的存储表示（UTC）。
pub fn now() -> String {
    format_ts(Utc::now())
}

/// 将 UTC 时刻格式化为存储表示。
pub fn format_ts(dt: DateTime<Utc>) -> String {
    dt.format(TS_FORMAT).to_string()
}

/// 解析存储的 UTC 时间戳。
pub fn parse_ts(s: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| format!("无法解析时间戳 `{s}`: {e}"))
}

/// 当前的本地自然日。
pub fn today() -> String {
    local_date_of(Utc::now())
}

/// UTC 时刻归属的本地自然日。
pub fn local_date_of(dt: DateTime<Utc>) -> String {
    local_date_in(dt, &Local)
}

/// 指定时区下，UTC 时刻归属的自然日。
///
/// 生产代码用 `local_date_of`（系统时区）；此函数供测试注入固定时区，
/// 使断言与运行机器的时区无关。
pub fn local_date_in<Tz: TimeZone>(dt: DateTime<Utc>, tz: &Tz) -> String {
    dt.with_timezone(tz).date_naive().format(DATE_FORMAT).to_string()
}

/// UTC 时刻加天数。负数为向前回溯。
pub fn add_days(dt: DateTime<Utc>, days: i64) -> DateTime<Utc> {
    if days >= 0 {
        dt.checked_add_days(Days::new(days as u64)).unwrap_or(dt)
    } else {
        dt.checked_sub_days(Days::new(days.unsigned_abs()))
            .unwrap_or(dt)
    }
}

/// 从现在起 `days` 天后的存储表示。用于设置 `due_at`。
pub fn due_in_days(days: f64) -> String {
    let secs = (days * 86_400.0).round() as i64;
    format_ts(Utc::now() + chrono::Duration::seconds(secs))
}

/// 某个本地自然日对应的 UTC 时间范围 `[start, end)`。
///
/// 日志按 UTC 存储却要按本地日聚合，两种做法都不可取：在 SQL 里用 `localtime`
/// 修饰符会依赖进程时区且无法单测；把全表读进内存再分组在数据量大时不可行。
/// 这里反过来——把本地日换算成一对 UTC 边界，SQL 仍走 `idx_logs_date` 索引。
pub fn local_day_bounds(date: &str) -> Result<(String, String), String> {
    local_day_bounds_in(date, &Local)
}

/// `local_day_bounds` 的可注入时区版本，供测试使用。
pub fn local_day_bounds_in<Tz: TimeZone>(
    date: &str,
    tz: &Tz,
) -> Result<(String, String), String> {
    let day = NaiveDate::parse_from_str(date, DATE_FORMAT)
        .map_err(|e| format!("无法解析日期 `{date}`: {e}"))?;

    let midnight = day
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| format!("无法构造 `{date}` 的零点"))?;

    // DST 切换日可能出现不存在或重复的本地时刻，取最早的有效映射
    let start = tz
        .from_local_datetime(&midnight)
        .earliest()
        .ok_or_else(|| format!("`{date}` 零点在本地时区不存在（DST 跳变）"))?
        .with_timezone(&Utc);

    Ok((format_ts(start), format_ts(start + chrono::Duration::days(1))))
}

/// 本地日期字符串所属的月份 `YYYY-MM`。用于补签卡月度发放与暂停配额重置。
/// 两个本地日期（`YYYY-MM-DD`）相差的天数，`to` 晚于 `from` 时为正。
///
/// 走 `NaiveDate` 而非字符串或秒数运算（ADR-4）——跨月、跨年、闰年在这里
/// 都是 chrono 的责任。
pub fn days_between(from: &str, to: &str) -> Result<i64, String> {
    let parse = |s: &str| {
        chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|e| format!("日期 `{s}` 解析失败: {e}"))
    };
    Ok((parse(to)? - parse(from)?).num_days())
}

pub fn month_of(date: &str) -> Result<String, String> {
    let d = NaiveDate::parse_from_str(date, DATE_FORMAT)
        .map_err(|e| format!("无法解析日期 `{date}`: {e}"))?;
    Ok(format!("{:04}-{:02}", d.year(), d.month()))
}

/// 当前本地月份 `YYYY-MM`。
pub fn current_month() -> String {
    Local::now().format("%Y-%m").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    /// UTC+8（中国标准时间）
    fn cst() -> FixedOffset {
        FixedOffset::east_opt(8 * 3600).unwrap()
    }

    /// UTC-5（美东标准时间）
    fn est() -> FixedOffset {
        FixedOffset::west_opt(5 * 3600).unwrap()
    }

    fn utc(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
    }

    #[test]
    fn 跨本地午夜的两个时刻分属不同自然日() {
        // UTC+8：15:59Z 是当地 23:59，16:01Z 是次日 00:01
        let before = utc(2026, 8, 5, 15, 59);
        let after = utc(2026, 8, 5, 16, 1);

        assert_eq!(local_date_in(before, &cst()), "2026-08-05");
        assert_eq!(local_date_in(after, &cst()), "2026-08-06");
        assert_ne!(
            local_date_in(before, &cst()),
            local_date_in(after, &cst()),
            "跨本地午夜的两条记录被归入了同一天"
        );

        // 同样这两个时刻，在 UTC 下都还是 8-05——这正是必须按本地时区归属的原因
        assert_eq!(local_date_in(before, &Utc), "2026-08-05");
        assert_eq!(local_date_in(after, &Utc), "2026-08-05");
    }

    #[test]
    fn 西半球时区下自然日向前偏移() {
        // UTC 8-05 02:00 在 UTC-5 是 8-04 21:00
        let dt = utc(2026, 8, 5, 2, 0);
        assert_eq!(local_date_in(dt, &est()), "2026-08-04");
        assert_eq!(local_date_in(dt, &Utc), "2026-08-05");
    }

    #[test]
    fn 时间戳往返无损() {
        let original = utc(2026, 8, 5, 12, 34);
        let s = format_ts(original);
        assert_eq!(s, "2026-08-05T12:34:00Z");
        assert_eq!(parse_ts(&s).unwrap(), original);
    }

    #[test]
    fn 解析非法时间戳返回错误而不崩溃() {
        assert!(parse_ts("").is_err());
        assert!(parse_ts("2026-13-45").is_err());
        assert!(parse_ts("not a timestamp").is_err());
        // 审计 D2 的手写实现会产出这种不存在的日期
        assert!(parse_ts("2026-13-01T00:00:00Z").is_err());
    }

    #[test]
    fn 加天数正确跨月与跨年() {
        // 审计 D2：手写实现按 30 天分月，无法正确处理 8 月 31 天
        let aug31 = utc(2026, 8, 31, 10, 0);
        assert_eq!(format_ts(add_days(aug31, 1)), "2026-09-01T10:00:00Z");

        let dec31 = utc(2026, 12, 31, 10, 0);
        assert_eq!(format_ts(add_days(dec31, 1)), "2027-01-01T10:00:00Z");

        // 闰年 2 月
        let feb28 = utc(2024, 2, 28, 10, 0);
        assert_eq!(format_ts(add_days(feb28, 1)), "2024-02-29T10:00:00Z");
        assert_eq!(format_ts(add_days(feb28, 2)), "2024-03-01T10:00:00Z");

        // 平年 2 月
        let feb28_2026 = utc(2026, 2, 28, 10, 0);
        assert_eq!(format_ts(add_days(feb28_2026, 1)), "2026-03-01T10:00:00Z");
    }

    #[test]
    fn 加天数支持负数回溯() {
        let day = utc(2026, 3, 1, 10, 0);
        assert_eq!(format_ts(add_days(day, -1)), "2026-02-28T10:00:00Z");
        assert_eq!(format_ts(add_days(day, 0)), "2026-03-01T10:00:00Z");
    }

    #[test]
    fn 加天数与天数参数真实相关() {
        // 审计 D1：原 add_days 丢弃 days 参数，7 天后与 1 天后返回同一日期
        let base = utc(2026, 8, 5, 0, 0);
        let one = format_ts(add_days(base, 1));
        let seven = format_ts(add_days(base, 7));
        assert_ne!(one, seven, "add_days 忽略了天数参数");
        assert_eq!(one, "2026-08-06T00:00:00Z");
        assert_eq!(seven, "2026-08-12T00:00:00Z");
    }

    #[test]
    fn 小数天数的到期时间按比例换算() {
        // FSRS 的 stability 是浮点天数，0.5 天应是 12 小时而非被截断为 0
        let before = Utc::now();
        let half = parse_ts(&due_in_days(0.5)).unwrap();
        let elapsed = half - before;
        assert!(
            (elapsed.num_minutes() - 720).abs() <= 1,
            "0.5 天应约等于 720 分钟，实际 {} 分钟",
            elapsed.num_minutes()
        );
    }

    #[test]
    fn 本地日边界换算为正确的_utc_区间() {
        // UTC+8：本地 8-05 00:00 是 UTC 8-04 16:00
        let (start, end) = local_day_bounds_in("2026-08-05", &cst()).unwrap();
        assert_eq!(start, "2026-08-04T16:00:00Z");
        assert_eq!(end, "2026-08-05T16:00:00Z");

        // UTC 下则与自然日重合
        let (start, end) = local_day_bounds_in("2026-08-05", &Utc).unwrap();
        assert_eq!(start, "2026-08-05T00:00:00Z");
        assert_eq!(end, "2026-08-06T00:00:00Z");
    }

    #[test]
    fn 边界区间恰好覆盖该本地日的全部时刻() {
        let (start, end) = local_day_bounds_in("2026-08-05", &cst()).unwrap();
        let start_dt = parse_ts(&start).unwrap();
        let end_dt = parse_ts(&end).unwrap();

        // 区间内的时刻都归属这一天
        for probe in [start_dt, start_dt + chrono::Duration::hours(12)] {
            assert_eq!(local_date_in(probe, &cst()), "2026-08-05");
        }
        // 上界是开区间：end 本身已属次日
        assert_eq!(local_date_in(end_dt, &cst()), "2026-08-06");
        // 下界前一秒属前一日
        assert_eq!(
            local_date_in(start_dt - chrono::Duration::seconds(1), &cst()),
            "2026-08-04"
        );
    }

    #[test]
    fn 非法日期的边界换算返回错误() {
        assert!(local_day_bounds_in("2026-13-01", &cst()).is_err());
        assert!(local_day_bounds_in("", &cst()).is_err());
    }

    #[test]
    fn 月份提取正确() {
        assert_eq!(month_of("2026-08-05").unwrap(), "2026-08");
        assert_eq!(month_of("2026-12-31").unwrap(), "2026-12");
        assert_eq!(month_of("2027-01-01").unwrap(), "2027-01");
        assert!(month_of("2026-13-01").is_err(), "非法月份未被拒绝");
        assert!(month_of("").is_err());
    }

    #[test]
    fn 当前日期与月份格式合法() {
        let today = today();
        assert!(
            NaiveDate::parse_from_str(&today, DATE_FORMAT).is_ok(),
            "today() 产出了非法日期 `{today}`"
        );
        let month = current_month();
        assert_eq!(month.len(), 7, "月份格式应为 YYYY-MM，实际 `{month}`");
        assert_eq!(&month[..4], &today[..4], "月份与日期的年份不一致");
    }
}
