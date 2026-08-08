//! 赛季积分与赛道推进。spec §4.2 F11。
//!
//! 纯逻辑，可穷举——积分算错会直接影响用户能兑换到什么，而这类偏差
//! 要累积好几周才会被察觉。

use chrono::{Datelike, NaiveDate};

/// 一周的时段总数：7 天 × 3 个传送门。赛道满格即此值。
pub const SESSIONS_PER_WEEK: i64 = 21;

/// 每完成一个时段的积分。
pub const POINTS_PER_SESSION: i64 = 10;

/// 完美周（21 个时段全完成）的额外奖励。
///
/// 给得足够显眼才有冲刺意义——50 分相当于额外五个时段，
/// 但只有真的一个不落才拿得到。
pub const PERFECT_WEEK_BONUS: i64 = 50;

/// 兑换项与价格。
///
/// 抽卡券定 30 分（三个时段）：一天练满就能换一张，回报即时可见。
/// 补签卡定 150 分（十五个时段）：它能挽回一次断签，太便宜会让 streak
/// 失去约束力。
pub const REDEEM_DRAW_TICKET: i64 = 30;
pub const REDEEM_MAKEUP_CARD: i64 = 150;

// 补签卡能挽回一次断签，定价过低会让 streak 失去约束力。
// 编译期检查：改价格表时构建直接失败，不必等到跑测试
const _: () = assert!(REDEEM_MAKEUP_CARD >= REDEEM_DRAW_TICKET * 4);
// 练满一周却换不到任何东西，赛道就没有奖励感
const _: () = assert!(
    SESSIONS_PER_WEEK * POINTS_PER_SESSION + PERFECT_WEEK_BONUS >= REDEEM_DRAW_TICKET
);

/// 该日期所在周的周一。
///
/// 走 chrono 而非手写取模（ADR-4）。周一为周首符合国内习惯，
/// 也与「周末结算」的说法一致。
pub fn week_start(date: NaiveDate) -> NaiveDate {
    date - chrono::Duration::days(date.weekday().num_days_from_monday() as i64)
}

/// 本周积分。
pub fn points_for(sessions_done: i64) -> i64 {
    let capped = sessions_done.clamp(0, SESSIONS_PER_WEEK);
    let base = capped * POINTS_PER_SESSION;
    if capped >= SESSIONS_PER_WEEK {
        base + PERFECT_WEEK_BONUS
    } else {
        base
    }
}

/// 赛车在赛道上的位置，0.0 到 1.0。
pub fn track_ratio(sessions_done: i64) -> f64 {
    (sessions_done as f64 / SESSIONS_PER_WEEK as f64).clamp(0.0, 1.0)
}

/// 兑换是否可负担。
pub fn can_afford(points: i64, cost: i64) -> bool {
    cost > 0 && points >= cost
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn 周一到周日归属同一周() {
        // 2026-08-03 是周一
        let monday = d(2026, 8, 3);
        for offset in 0..7 {
            let day = monday + chrono::Duration::days(offset);
            assert_eq!(week_start(day), monday, "{day} 的周首算错");
        }
        // 下周一属于新的一周
        assert_eq!(week_start(monday + chrono::Duration::days(7)), d(2026, 8, 10));
    }

    #[test]
    fn 跨年周的归属正确() {
        // 2025-12-29 是周一，跨到 2026-01-04 周日仍属同一周。
        // 用 ISO 周数会在这里撞上 W53 与 W01 的边界问题
        assert_eq!(week_start(d(2025, 12, 31)), d(2025, 12, 29));
        assert_eq!(week_start(d(2026, 1, 4)), d(2025, 12, 29));
        assert_eq!(week_start(d(2026, 1, 5)), d(2026, 1, 5));
    }

    #[test]
    fn 积分按完成时段线性累加() {
        assert_eq!(points_for(0), 0);
        assert_eq!(points_for(1), 10);
        assert_eq!(points_for(10), 100);
    }

    #[test]
    fn 完美周有额外奖励() {
        // 20 个时段与 21 个之间要有明显落差，冲刺才有意义
        assert_eq!(points_for(20), 200);
        assert_eq!(points_for(21), 210 + PERFECT_WEEK_BONUS);
    }

    #[test]
    fn 超出一周上限的时段不重复计分() {
        // free 时段不计入 streak，但可能被误统计进来；封顶防止刷分
        assert_eq!(points_for(30), points_for(21));
        assert_eq!(points_for(100), points_for(21));
    }

    #[test]
    fn 负数时段不产生负积分() {
        assert_eq!(points_for(-5), 0);
    }

    #[test]
    fn 赛道进度落在零到一之间() {
        assert_eq!(track_ratio(0), 0.0);
        assert!((track_ratio(21) - 1.0).abs() < 1e-9);
        // 超额不会让赛车冲出赛道
        assert!((track_ratio(50) - 1.0).abs() < 1e-9);
        assert_eq!(track_ratio(-3), 0.0);
    }

    #[test]
    fn 兑换需要足额积分() {
        assert!(can_afford(30, REDEEM_DRAW_TICKET));
        assert!(!can_afford(29, REDEEM_DRAW_TICKET));
        assert!(can_afford(150, REDEEM_MAKEUP_CARD));
        assert!(!can_afford(149, REDEEM_MAKEUP_CARD));
    }

    #[test]
    fn 零价与负价不可兑换() {
        // 防止价格表配错导致无限兑换
        assert!(!can_afford(1000, 0));
        assert!(!can_afford(1000, -10));
    }

}
