//! 赛季积分与赛道推进。spec §4.2 F11。
//!
//! 纯逻辑，可穷举——积分算错会直接影响用户能兑换到什么，而这类偏差
//! 要累积好几周才会被察觉。

use chrono::{Datelike, NaiveDate};

/// 每天的传送门数。
pub const SESSIONS_PER_DAY: i64 = 3;

/// 每天都学时的一周时段总数。仅作参考上限与编译期校验用——
/// **实际分母按用户设定的学习日数算**（见 `season::state_of`）：
/// 开学后只有周末能用电脑，21 这个目标从第一天起就够不着，
/// 而够不着的目标不激励人，只会让人不再看它。
pub const SESSIONS_PER_WEEK: i64 = SESSIONS_PER_DAY * 7;

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

/// 本周积分。`total` 是本周实际可得的时段数（学习日数 × 3）。
///
/// 完美奖励按 `total` 判定而非写死的 21：只在周末学的用户练满 6 个时段
/// 就是他的「一个不落」，拿不到这份奖励等于惩罚他上学。
pub fn points_for_total(sessions_done: i64, total: i64) -> i64 {
    let total = total.max(1);
    let capped = sessions_done.clamp(0, total);
    let base = capped * POINTS_PER_SESSION;
    if capped >= total {
        base + PERFECT_WEEK_BONUS
    } else {
        base
    }
}

/// 赛车位置，0.0 到 1.0。
pub fn ratio_of(sessions_done: i64, total: i64) -> f64 {
    (sessions_done as f64 / total.max(1) as f64).clamp(0.0, 1.0)
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
        assert_eq!(points_for_total(0, SESSIONS_PER_WEEK), 0);
        assert_eq!(points_for_total(1, SESSIONS_PER_WEEK), 10);
        assert_eq!(points_for_total(10, SESSIONS_PER_WEEK), 100);
    }

    #[test]
    fn 完美周有额外奖励() {
        // 20 个时段与 21 个之间要有明显落差，冲刺才有意义
        assert_eq!(points_for_total(20, SESSIONS_PER_WEEK), 200);
        assert_eq!(points_for_total(21, SESSIONS_PER_WEEK), 210 + PERFECT_WEEK_BONUS);
    }

    #[test]
    fn 超出一周上限的时段不重复计分() {
        // free 时段不计入 streak，但可能被误统计进来；封顶防止刷分
        assert_eq!(points_for_total(30, SESSIONS_PER_WEEK), points_for_total(21, SESSIONS_PER_WEEK));
        assert_eq!(points_for_total(100, SESSIONS_PER_WEEK), points_for_total(21, SESSIONS_PER_WEEK));
    }

    #[test]
    fn 负数时段不产生负积分() {
        assert_eq!(points_for_total(-5, SESSIONS_PER_WEEK), 0);
    }

    #[test]
    fn 赛道进度落在零到一之间() {
        assert_eq!(ratio_of(0, SESSIONS_PER_WEEK), 0.0);
        assert!((ratio_of(21, SESSIONS_PER_WEEK) - 1.0).abs() < 1e-9);
        // 超额不会让赛车冲出赛道
        assert!((ratio_of(50, SESSIONS_PER_WEEK) - 1.0).abs() < 1e-9);
        assert_eq!(ratio_of(-3, SESSIONS_PER_WEEK), 0.0);
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


#[cfg(test)]
mod study_day_tests {
    use super::*;

    #[test]
    fn 完美奖励按实际可得时段判定() {
        // 只在周末学的人，练满 6 个时段就是他的「一个不落」。
        // 按写死的 21 判，他永远拿不到这份奖励——等于惩罚他上学
        assert_eq!(points_for_total(6, 6), 6 * POINTS_PER_SESSION + PERFECT_WEEK_BONUS);
        assert_eq!(points_for_total(5, 6), 5 * POINTS_PER_SESSION);
    }

    #[test]
    fn 超出总数不重复计分() {
        // free 时段等原因可能让完成数超过分母
        assert_eq!(points_for_total(30, 6), points_for_total(6, 6));
    }

    #[test]
    fn 分母为零不崩也不除零() {
        // 设置被写坏时不该让整个赛道页失败
        assert!(points_for_total(3, 0) >= 0);
        assert!(ratio_of(3, 0).is_finite());
    }

    #[test]
    fn 比例按实际分母算() {
        assert_eq!(ratio_of(3, 6), 0.5);
        assert_eq!(ratio_of(6, 6), 1.0);
        // 每天都学的旧口径仍然成立
        assert_eq!(ratio_of(21, SESSIONS_PER_WEEK), 1.0);
    }
}
}
