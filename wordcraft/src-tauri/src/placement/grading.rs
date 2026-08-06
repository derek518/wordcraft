//! 摸底判定与预分级规则。contracts §9.2③ / §9.3。
//!
//! 纯逻辑，不碰数据库——这里的每条规则都直接决定用户开局要学多少词，
//! 算错会让人白学几百个已经会的词，或把不会的词当会跳过。

/// 「已会」的反应时间上限（§9.3 防猜）。
///
/// 四选一有 25% 基础猜对率，基础薄弱者实测常达 30–40%。只看正误会把猜对
/// 当掌握，因此叠加时间条件——蒙对的人不会在 4 秒内蒙对。
pub const PASS_REACTION_MS: i64 = 4000;

/// 每层的目标题量（§9.2②：1600 词分 5 层，每层约 12 题）。
pub const QUESTIONS_PER_BAND: i64 = 12;

/// 连续答错多少题就提前结束（§9.2②）。
pub const CONSECUTIVE_MISS_LIMIT: i64 = 3;

/// 摸底范围：只测初中词。
///
/// §9.2①：`senior` 词默认全部 new（新高一大概率未系统学过），
/// `art` 词随 zone `rock` 解锁。范围缩到约 1600 词后，同样 5 分钟能得到
/// 细得多的粒度。
pub const PLACEMENT_LEVEL: &str = "junior";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreGrade {
    /// p > 0.85：判定已掌握，跳过新词队列
    Review,
    /// 0.5 < p ≤ 0.85：学过但不牢
    Learning,
    /// p ≤ 0.5：当作新词正常排队
    New,
}

impl PreGrade {
    pub fn app_state(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::Learning => "learning",
            Self::New => "new",
        }
    }

    pub fn question_level(self) -> i64 {
        match self {
            Self::Review => 2,
            _ => 1,
        }
    }
}

/// 单题是否算作「已会」。
pub fn is_pass(is_correct: bool, reaction_ms: i64) -> bool {
    is_correct && reaction_ms < PASS_REACTION_MS
}

/// 按层掌握率决定预分级（§9.2③ 表格）。
pub fn grade_for(pass_rate: f64) -> PreGrade {
    if pass_rate > 0.85 {
        PreGrade::Review
    } else if pass_rate > 0.5 {
        PreGrade::Learning
    } else {
        PreGrade::New
    }
}

/// 预分级词的 stability 初值区间（天）。
///
/// **分层抖动是必需的，不是锦上添花**：若 960 个词全赋 stability=14，
/// 14 天后它们会在同一天集中到期，把每日 20 词次的预算彻底淹没。
///
/// 高频词给短间隔——它们价值最高，早一点验证摸底判断是否正确；
/// 低频词给长间隔，假阳性可以晚些暴露。
pub fn stability_range(band: i64) -> (f64, f64) {
    match band {
        1 | 2 => (7.0, 30.0),
        3 | 4 => (30.0, 90.0),
        _ => (90.0, 180.0),
    }
}

/// 估算词汇量：各层掌握率 × 该层词数之和。
///
/// 未测到的层按 0 计——没有证据就不算掌握，宁可少估。高估会让用户
/// 跳过实际不会的词，那是不可逆的损失；低估只是多学几遍。
pub fn estimate_vocab(band_totals: &[(i64, i64)], pass_rates: &[(i64, f64)]) -> i64 {
    band_totals
        .iter()
        .map(|(band, total)| {
            let rate = pass_rates
                .iter()
                .find(|(b, _)| b == band)
                .map(|(_, r)| *r)
                .unwrap_or(0.0);
            (*total as f64 * rate).round() as i64
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 答对但超时不算已会() {
        // §9.3：四选一 25% 基础猜对率，只看正误会把蒙对当掌握
        assert!(is_pass(true, 3999));
        assert!(!is_pass(true, 4000), "4 秒是上限，取不到");
        assert!(!is_pass(true, 10_000));
    }

    #[test]
    fn 答错一律不算已会() {
        assert!(!is_pass(false, 100));
        assert!(!is_pass(false, 99_999));
    }

    #[test]
    fn 掌握率分档与契约表一致() {
        assert_eq!(grade_for(1.0), PreGrade::Review);
        assert_eq!(grade_for(0.86), PreGrade::Review);
        assert_eq!(grade_for(0.85), PreGrade::Learning, "0.85 属于中档");
        assert_eq!(grade_for(0.51), PreGrade::Learning);
        assert_eq!(grade_for(0.5), PreGrade::New, "0.5 属于低档");
        assert_eq!(grade_for(0.0), PreGrade::New);
    }

    #[test]
    fn 预分级映射到正确的状态与题型() {
        assert_eq!(PreGrade::Review.app_state(), "review");
        assert_eq!(PreGrade::Review.question_level(), 2);
        assert_eq!(PreGrade::Learning.app_state(), "learning");
        assert_eq!(PreGrade::Learning.question_level(), 1);
        assert_eq!(PreGrade::New.app_state(), "new");
    }

    #[test]
    fn 稳定性区间按频段分层且互不重叠() {
        let (lo1, hi1) = stability_range(1);
        let (lo3, hi3) = stability_range(3);
        let (lo5, hi5) = stability_range(5);

        assert_eq!((lo1, hi1), (7.0, 30.0));
        assert_eq!((lo3, hi3), (30.0, 90.0));
        assert_eq!((lo5, hi5), (90.0, 180.0));

        // 高频词间隔更短——价值最高，应当早验证
        assert!(hi1 <= lo3);
        assert!(hi3 <= lo5);
    }

    #[test]
    fn 抖动区间足够宽以避免集中到期() {
        // 960 词全赋同一 stability 会在同一天集中到期，
        // 把每日 20 词次的预算淹没
        for band in 1..=5 {
            let (lo, hi) = stability_range(band);
            assert!(hi - lo >= 20.0, "band {band} 的区间宽度不足以分散到期");
        }
    }

    #[test]
    fn 词汇量估算按层加权() {
        let totals = [(1, 400), (2, 400), (3, 400), (4, 200), (5, 200)];
        let rates = [(1, 1.0), (2, 0.5), (3, 0.0), (4, 0.0), (5, 0.0)];
        // 400×1.0 + 400×0.5 = 600
        assert_eq!(estimate_vocab(&totals, &rates), 600);
    }

    #[test]
    fn 未测层按零计而非按平均值外推() {
        // 没有证据就不算掌握。高估会让用户跳过实际不会的词——
        // 那是不可逆的损失，低估只是多学几遍
        let totals = [(1, 100), (2, 100)];
        let rates = [(1, 1.0)];
        assert_eq!(estimate_vocab(&totals, &rates), 100);
    }

    #[test]
    fn 空输入返回零而非报错() {
        assert_eq!(estimate_vocab(&[], &[]), 0);
    }
}
