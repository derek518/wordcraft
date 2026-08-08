//! 方块发放规则。plan: docs/plans/homestead-v1.1.md §4。
//!
//! 纯逻辑，不碰数据库——发放数量直接决定用户能建多大的家园，算错要么
//! 白送一千块让建造失去意义，要么该给的不给。这类规则必须能穷举验证。

/// 里程碑阈值（累计实际作答词数）。跨过一个发一块稀有方块。
///
/// spec 中稀有方块的来源是「击败魔王」（F10 未实现）。里程碑暂代其来源，
/// F10 落地后两者并存——发放共用一张账本，`source` 区分即可。
pub const MILESTONES: [i64; 5] = [200, 500, 1000, 2000, 3657];

/// 连续打卡每满这个天数发一块限定方块。
pub const STREAK_STEP: i64 = 7;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingGrant {
    pub source: &'static str,
    pub source_key: String,
    pub block_type: &'static str,
    pub amount: i64,
}

/// 已作答的词应发的方块。`already` 是账本里已发过的 word_id。
///
/// 只看实际作答过的词（`reps > 0`），不看是否有 `word_states` 行——
/// 摸底会为一千多个词预建状态，那是「估计你可能认识」，不是「你收集到了」。
/// 按后者发放，用户做完摸底立刻凭空得到一千多块，建造第一天就失去意义。
pub fn mastery_grants(answered_word_ids: &[i64], already: &[String]) -> Vec<PendingGrant> {
    answered_word_ids
        .iter()
        .map(|id| id.to_string())
        .filter(|key| !already.contains(key))
        .map(|key| PendingGrant {
            source: "mastery",
            source_key: key,
            block_type: "normal",
            amount: 1,
        })
        .collect()
}

/// 连续打卡应发的限定方块。
///
/// 按 7 的倍数逐个发，而非只发最新一档：用户可能在离线期间连到 21 天，
/// 一次只补一块会永久少发两块。
pub fn streak_grants(best_streak: i64, already: &[String]) -> Vec<PendingGrant> {
    (1..=best_streak / STREAK_STEP)
        .map(|n| (n * STREAK_STEP).to_string())
        .filter(|key| !already.contains(key))
        .map(|key| PendingGrant {
            source: "streak",
            source_key: key,
            block_type: "limited",
            amount: 1,
        })
        .collect()
}

/// 词量里程碑应发的稀有方块。同样逐个补，摸底后可能一次跨过多个阈值。
pub fn milestone_grants(answered_count: i64, already: &[String]) -> Vec<PendingGrant> {
    MILESTONES
        .iter()
        .filter(|m| answered_count >= **m)
        .map(|m| m.to_string())
        .filter(|key| !already.contains(key))
        .map(|key| PendingGrant {
            source: "milestone",
            source_key: key,
            block_type: "rare",
            amount: 1,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(grants: &[PendingGrant]) -> Vec<&str> {
        grants.iter().map(|g| g.source_key.as_str()).collect()
    }

    // ── mastery ──────────────────────────

    #[test]
    fn 每个作答过的词发一块普通方块() {
        let g = mastery_grants(&[1, 2, 3], &[]);
        assert_eq!(g.len(), 3);
        assert!(g.iter().all(|x| x.block_type == "normal" && x.amount == 1));
    }

    #[test]
    fn 已发放的词不再重复发() {
        // 发放在每次启动与每次会话结束时触发，重复是常态而非例外
        let g = mastery_grants(&[1, 2, 3], &["1".into(), "2".into()]);
        assert_eq!(keys(&g), vec!["3"]);
    }

    #[test]
    fn 全部已发时返回空() {
        let g = mastery_grants(&[1, 2], &["1".into(), "2".into()]);
        assert!(g.is_empty());
    }

    #[test]
    fn 无作答记录时不发方块() {
        // 摸底预分级的词 reps=0，不在入参里——它们不该产生方块
        assert!(mastery_grants(&[], &[]).is_empty());
    }

    // ── streak ──────────────────────────

    #[test]
    fn 连续七天发一块限定方块() {
        assert!(streak_grants(6, &[]).is_empty());
        assert_eq!(keys(&streak_grants(7, &[])), vec!["7"]);
        assert_eq!(keys(&streak_grants(13, &[])), vec!["7"]);
    }

    #[test]
    fn 跨越多个七天档位时逐个补发() {
        // 离线期间连到 21 天，只补最新一档会永久少发两块
        assert_eq!(keys(&streak_grants(21, &[])), vec!["7", "14", "21"]);
        assert_eq!(keys(&streak_grants(30, &[])), vec!["7", "14", "21", "28"]);
    }

    #[test]
    fn 已发的档位跳过() {
        let g = streak_grants(21, &["7".into(), "14".into()]);
        assert_eq!(keys(&g), vec!["21"]);
    }

    #[test]
    fn 零与负数连续天数不发放() {
        assert!(streak_grants(0, &[]).is_empty());
        assert!(streak_grants(-3, &[]).is_empty());
    }

    // ── milestone ──────────────────────────

    #[test]
    fn 跨过阈值发稀有方块() {
        assert!(milestone_grants(199, &[]).is_empty());
        assert_eq!(keys(&milestone_grants(200, &[])), vec!["200"]);
        assert_eq!(keys(&milestone_grants(499, &[])), vec!["200"]);
    }

    #[test]
    fn 一次跨过多个阈值时全部发放() {
        // 摸底后可能从 0 直接跳到几百词
        assert_eq!(keys(&milestone_grants(1200, &[])), vec!["200", "500", "1000"]);
    }

    #[test]
    fn 学完全部词库发满五块() {
        assert_eq!(milestone_grants(3657, &[]).len(), MILESTONES.len());
    }

    #[test]
    fn 已发的里程碑跳过() {
        let g = milestone_grants(1200, &["200".into(), "500".into()]);
        assert_eq!(keys(&g), vec!["1000"]);
    }

    // ── 交叉 ──────────────────────────

    #[test]
    fn 三种来源的键互不干扰() {
        // mastery 的 "200" 是 word_id，milestone 的 "200" 是词数阈值。
        // 数据库靠 (source, source_key) 联合唯一区分，逻辑层也不能混
        let m = mastery_grants(&[200], &[]);
        let s = milestone_grants(200, &[]);
        assert_eq!(m[0].source, "mastery");
        assert_eq!(s[0].source, "milestone");
        assert_eq!(m[0].block_type, "normal");
        assert_eq!(s[0].block_type, "rare");
    }

    #[test]
    fn 里程碑阈值单调递增() {
        for pair in MILESTONES.windows(2) {
            assert!(pair[0] < pair[1], "阈值未递增: {MILESTONES:?}");
        }
    }
}
