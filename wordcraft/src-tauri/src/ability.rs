//! 能力估计：孩子的词汇水平在**词频轴**上的位置。
//!
//! 纯逻辑，不碰数据库。持久化在 `db::repo::player_stats`。
//!
//! ## 为什么是词频
//!
//! 词汇掌握对词频高度单调：会 `abandon`（第 2500 名左右）的人几乎必然会
//! `the`（第 1 名）。所有词汇量测试都建立在这个规律上。于是「这孩子会不会
//! 某个没见过的词」可以用一个数回答——他的掌握边界落在词频轴的哪一位。
//!
//! 这取代了手选「初中 / 高中 / 四级」。那三个标签和难度基本无关：102 个高中词
//! 的常用度和 `the` 同级，28 个初中词比大多数四级词还生僻。用标签筛选，等于
//! 既在练已经会的词，又在漏掉该练的词。
//!
//! ## 模型
//!
//! 三参数 logistic（IRT 的 3PL，判别度固定）：
//!
//! ```text
//! 难度  d = log2(词频排名)
//! 能力  θ 同尺度——θ = 11 意味着「第 2048 名前后的词有一半把握」
//! 答对  P = c + (1-c)·σ((θ-d)/s)
//! ```
//!
//! `c` 是四选一的猜对率。**它必须在模型里**：只看正误会把蒙对当掌握，
//! 而基础薄弱者的实测猜对率能到 30–40%。
//!
//! ## 更新时机：只在**首次遇见**
//!
//! 第五次复习 `abandon` 答对，说明的是「这个应用把它教会了」，不是
//! 「这孩子本来就会」。用复习结果更新 θ 会让估计随训练虚高，然后系统
//! 开始跳过它其实没教过的词。
//!
//! 首次作答才是关于**基线词汇量**的观测。应用教会的词由 FSRS 逐词跟踪，
//! 不走 θ。
//!
//! ## 收敛与推进
//!
//! 更新用 Fisher 记分法，步长是「本次观测的信息量 ÷ 累计信息量」——
//! 前几次观测大幅修正，几十次之后自然稳定，不需要人为衰减系数。
//!
//! θ 稳定后学习不会停滞：候选只在**没学过**的词里选，边界附近的词学完了，
//! 池子自然向更难处推进。进步靠词池消耗，不靠 θ 漂移。

/// logistic 斜率。
///
/// **未校准**——这是先验值，不是实测。它决定「已掌握 / 前沿 / 太超前」的
/// 分界宽窄，真值只能由孩子的实际作答估出来（见 `calibrate_slope`）。
/// 在拿到足够观测之前，这里保守取一个较平缓的值：斜率取平会让前沿变宽，
/// 宁可多练几个已经会的词，也不要把不会的词判成会。
pub const SLOPE: f64 = 1.1;

/// 四选一的基础猜对率。
pub const GUESS: f64 = 0.25;

/// 冷启动的能力先验：约第 2500 名。
///
/// 高考考纲 3500 词，目标用户是新高一。取 2500 而非 3500 是留出余量——
/// 低估只是多练几个已经会的词，高估会让孩子一上来就撞上不会的词。
pub const PRIOR_THETA: f64 = 11.29; // log2(2500)

/// 先验的信息量，约等于 16 次观测。
///
/// 由模拟选定，不是拍脑袋。设真实水平第 4000 名、先验第 2500 名，跑 60 组
/// 随机作答看头 10 次的估计波动与收敛速度：
///
/// ```text
/// 先验    头 10 次波动    30 次后    120 次后
///  0.5    1.10 log2      3515       4006
///  1.5    0.57 log2      3444       3952
///  2.0    0.46 log2      3324       3982     ← 取这个
///  4.0    0.27 log2      3142       3761     （收敛明显变慢）
/// ```
///
/// 取 0.5 时头几场的难度会在 ±2 倍排名之间乱跳——孩子感受到的是「这软件
/// 一会儿简单一会儿难得离谱」。取 4.0 又要上百次才显现真实水平。
pub const PRIOR_INFORMATION: f64 = 2.0;

/// θ 的取值范围：第 1 名到第 65536 名。
const THETA_MIN: f64 = 0.0;
const THETA_MAX: f64 = 16.0;

/// 已掌握的判定线。超过这个概率就不再排入新词队列。
const KNOWN_THRESHOLD: f64 = 0.85;

/// 太超前的判定线。低于这个概率暂缓——不是永远不学，是先学边界上的。
const REACH_THRESHOLD: f64 = 0.30;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ability {
    /// 能力值，log2(词频排名) 尺度
    pub theta: f64,
    /// 累计 Fisher 信息量。越大表示估计越稳，单次观测的影响越小
    pub information: f64,
    /// 参与估计的观测数（仅首次作答）
    pub observations: i64,
}

impl Default for Ability {
    fn default() -> Self {
        Self {
            theta: PRIOR_THETA,
            information: PRIOR_INFORMATION,
            observations: 0,
        }
    }
}

/// 词的难度。
pub fn difficulty(rank: i64) -> f64 {
    (rank.max(1) as f64).log2()
}

fn sigma(z: f64) -> f64 {
    1.0 / (1.0 + (-z).exp())
}

/// 答对概率，含猜测下限。
pub fn p_correct(theta: f64, rank: i64) -> f64 {
    GUESS + (1.0 - GUESS) * sigma((theta - difficulty(rank)) / SLOPE)
}

/// **真的会**的概率——剔除猜测成分。
///
/// 内容筛选用这个而非 `p_correct`：后者对完全不会的词也给 0.25，
/// 拿它当门槛会把「四分之一能蒙对」误读成「有点印象」。
pub fn p_known(theta: f64, rank: i64) -> f64 {
    sigma((theta - difficulty(rank)) / SLOPE)
}

/// 词相对当前能力的位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// 大概率已经会，不必再教
    Known,
    /// 学习前沿——该练的就是这些
    Frontier,
    /// 暂时太难，等边界推过去再说
    TooHard,
}

pub fn tier(theta: f64, rank: i64) -> Tier {
    let p = p_known(theta, rank);
    if p > KNOWN_THRESHOLD {
        Tier::Known
    } else if p >= REACH_THRESHOLD {
        Tier::Frontier
    } else {
        Tier::TooHard
    }
}

/// 前沿对应的词频排名区间 `[最高频, 最低频]`。
///
/// 供排队与界面展示。由阈值反解，不另写一份数字——
/// 阈值改了而区间没跟着改，界面就开始说谎。
pub fn frontier_ranks(theta: f64) -> (i64, i64) {
    let rank_at = |p: f64| {
        let d = theta - SLOPE * (p / (1.0 - p)).ln();
        d.exp2().clamp(1.0, i64::MAX as f64).round() as i64
    };
    // p 越大越简单 → 排名越靠前
    (rank_at(KNOWN_THRESHOLD), rank_at(REACH_THRESHOLD))
}

/// 一次首见作答后的能力更新（Fisher 记分法）。
///
/// 步长 = 本次得分 ÷ 累计信息量。前几次观测大幅修正，之后自然收敛，
/// 不需要人为衰减系数——那种系数是另一个没法校准的常量。
pub fn update(prior: Ability, rank: i64, is_correct: bool) -> Ability {
    let q = p_known(prior.theta, rank);
    let p = p_correct(prior.theta, rank);

    // 概率贴到 0 或 1 时分母塌陷。夹住而不是跳过——
    // 极端难度的词信息量本就接近 0，夹住后自然只贡献很小的修正
    let p = p.clamp(1e-6, 1.0 - 1e-6);
    let dp = (1.0 - GUESS) * q * (1.0 - q) / SLOPE;
    let variance = p * (1.0 - p);

    let y = if is_correct { 1.0 } else { 0.0 };
    let score = (y - p) / variance * dp;
    let info = dp * dp / variance;

    let information = prior.information + info;
    let theta = (prior.theta + score / information).clamp(THETA_MIN, THETA_MAX);

    Ability {
        theta,
        information,
        observations: prior.observations + 1,
    }
}

/// 估计的标准误，单位与 θ 相同（log2 排名）。
///
/// 用来告诉界面「这个估计有多确定」。SE 为 0.5 意味着真实水平大致在
/// 估计值的 ±41% 排名区间内——一次性摸底做不到更准，靠的是日积月累。
pub fn standard_error(information: f64) -> f64 {
    if information <= 0.0 {
        f64::INFINITY
    } else {
        information.sqrt().recip()
    }
}

/// θ 对应的词频排名边界：排名在此之前的词大概率已经会。
pub fn vocabulary_rank(theta: f64) -> i64 {
    theta.exp2().clamp(1.0, i64::MAX as f64).round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(mut a: Ability, obs: &[(i64, bool)]) -> Ability {
        for (rank, ok) in obs {
            a = update(a, *rank, *ok);
        }
        a
    }

    #[test]
    fn 难度随词频排名单调上升() {
        assert!(difficulty(1) < difficulty(1000));
        assert!(difficulty(1000) < difficulty(20000));
        // log2 尺度：排名翻倍等于难度加一
        assert!((difficulty(2000) - difficulty(1000) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn 排名非法时不恐慌也不产生负难度() {
        // 词库里 18 个词没有排名，取值路径上任何一处漏判都会走到这里
        assert_eq!(difficulty(0), 0.0);
        assert_eq!(difficulty(-5), 0.0);
    }

    #[test]
    fn 猜测下限使答对概率不低于四分之一() {
        let a = Ability::default();
        // 一个远超能力的词
        let p = p_correct(a.theta, 40000);
        assert!(p >= GUESS, "四选一的答对概率不该低于猜对率，得到 {p}");
        // 但「真的会」应当接近 0——用 p_correct 当门槛会把蒙对读成有印象
        assert!(p_known(a.theta, 40000) < 0.05);
    }

    /// 更新公式的定值测试。
    ///
    /// 步长的分母必须是**后验**信息量（先验 + 本次观测），不是先验本身。
    /// 两者都随观测增多而衰减，行为测试区分不出来，但用先验分母会系统性
    /// 过冲：首步大 24.8%，连续 30 次同向观测后估计从第 55212 名跑到
    /// 第 77048 名。这里把数值钉死。
    ///
    /// 推导（θ=PRIOR_THETA，rank=2500，答对）：
    ///   d = log2(2500) = 11.28771,  z = (θ-d)/SLOPE,  q = σ(z)
    ///   p = GUESS + (1-GUESS)·q = 0.625390
    ///   dp/dθ = (1-GUESS)·q(1-q)/SLOPE = 0.170454
    ///   score = (1-p)/(p(1-p))·dp = 0.272549
    ///   info  = dp²/(p(1-p))      = 0.124012
    ///   θ' = θ + score/(PRIOR_INFORMATION + info)
    #[test]
    fn 更新步长的分母是后验信息量() {
        let a = Ability::default();
        let b = update(a, 2500, true);

        assert!(
            (b.information - (PRIOR_INFORMATION + 0.124012)).abs() < 1e-4,
            "后验信息量 {:.6} 与推导不符",
            b.information
        );
        let expected = PRIOR_THETA + 0.272549 / (PRIOR_INFORMATION + 0.124012);
        assert!(
            (b.theta - expected).abs() < 1e-4,
            "θ 应为 {expected:.6}，得到 {:.6}——分母可能用成了先验",
            b.theta
        );
        // 用先验分母会更大。这一条直接排除那个写法
        assert!(
            b.theta - a.theta < 0.272549 / PRIOR_INFORMATION - 1e-6,
            "步长未被本次观测的信息量damp住"
        );
    }

    #[test]
    fn 答对提升估计答错降低估计() {
        let base = Ability::default();
        let up = update(base, 3000, true);
        let down = update(base, 3000, false);
        assert!(up.theta > base.theta);
        assert!(down.theta < base.theta);
        assert_eq!(up.observations, 1);
    }

    #[test]
    fn 观测越多单次影响越小() {
        let early = Ability::default();
        let d1 = (update(early, 3000, true).theta - early.theta).abs();

        // 喂一批混合观测，让信息量累积
        let late = feed(early, &[(3000, true), (3000, false), (2000, true), (4000, false)].repeat(15));
        let d2 = (update(late, 3000, true).theta - late.theta).abs();

        // 步长靠累计信息量自然衰减，不靠人为系数
        // 先验刻意值 16 次观测（见 PRIOR_INFORMATION），所以比值不会很夸张——
        // 换来的是头几场难度不乱跳
        assert!(d2 < d1 / 3.0, "第一次移动 {d1:.3}，稳定后仍移动 {d2:.3}");
        assert!(late.information > early.information * 3.0);
    }

    #[test]
    fn 远超能力的词贡献远小于边界上的词() {
        let base = Ability::default();
        // 第 40000 名的词答对多半是蒙的，不该和边界词一样有分量
        let extreme = (update(base, 40000, true).theta - base.theta).abs();
        let near = (update(base, 2500, true).theta - base.theta).abs();
        assert!(extreme < near / 3.0, "极端难度贡献 {extreme:.4}，边界贡献 {near:.4}");

        // 有观测积累之后应当近乎无影响。用默认值（先验只值 4 次观测）测这件事
        // 不严谨：那时任何一次观测都会大幅移动估计
        let settled = feed(base, &[(2500, true), (2500, false)].repeat(40));
        let e2 = (update(settled, 40000, true).theta - settled.theta).abs();
        assert!(e2 < 0.02, "稳定后极端观测仍移动 {e2:.4}");
    }

    #[test]
    fn 持续答对会收敛到更高水平() {
        // 一个实际水平远高于先验的孩子：边界附近的词一路答对
        let mut a = Ability::default();
        for _ in 0..40 {
            let rank = vocabulary_rank(a.theta);
            a = update(a, rank, true);
        }
        assert!(
            a.theta > PRIOR_THETA + 2.0,
            "40 次首见全对后 θ 只到 {:.2}（先验 {PRIOR_THETA:.2}）",
            a.theta
        );
    }

    #[test]
    fn 估计能收敛到真实水平() {
        // 真实水平第 6000 名，先验第 2500 名——模拟作答看能否走过去
        let truth = difficulty(6000);
        let mut a = Ability::default();
        // 固定序列而非随机：测试必须可复现
        for i in 0..120 {
            let rank = vocabulary_rank(a.theta);
            let p = GUESS + (1.0 - GUESS) * sigma((truth - difficulty(rank)) / SLOPE);
            // 用确定性的分数序列近似伯努利抽样
            let ok = ((i * 7) % 100) as f64 / 100.0 < p;
            a = update(a, rank, ok);
        }
        let est = vocabulary_rank(a.theta);
        assert!(
            (3000..12000).contains(&est),
            "真实第 6000 名，估到第 {est} 名——收敛失败"
        );
    }

    #[test]
    fn 分层与前沿区间彼此一致() {
        let theta = PRIOR_THETA;
        let (easy, hard) = frontier_ranks(theta);
        assert!(easy < hard, "前沿区间应从高频到低频：{easy}..{hard}");

        // 区间反解自阈值。两处各写一份数字，改了阈值界面就开始说谎
        assert_eq!(tier(theta, easy + 1), Tier::Frontier);
        assert_eq!(tier(theta, hard - 1), Tier::Frontier);
        assert_eq!(tier(theta, easy / 2), Tier::Known);
        assert_eq!(tier(theta, hard * 4), Tier::TooHard);
    }

    #[test]
    fn 前沿随能力上移() {
        let (low_easy, low_hard) = frontier_ranks(difficulty(1000));
        let (high_easy, high_hard) = frontier_ranks(difficulty(8000));
        // 水平高的孩子该练更生僻的词，而不是同一批
        assert!(high_easy > low_easy);
        assert!(high_hard > low_hard);
    }

    #[test]
    fn 标准误随观测下降() {
        let a = Ability::default();
        let se0 = standard_error(a.information);
        let b = feed(a, &[(2500, true), (2500, false)].repeat(30));
        assert!(
            standard_error(b.information) < se0 / 2.0,
            "60 次观测后标准误 {:.3}（起始 {se0:.3}）",
            standard_error(b.information)
        );
        // 没有任何信息时不该谎称精确
        assert_eq!(standard_error(0.0), f64::INFINITY);
    }

    #[test]
    fn 极端连续作答不会让估计跑飞() {
        let all_right = feed(Ability::default(), &[(1, true); 200]);
        let all_wrong = feed(Ability::default(), &[(40000, false); 200]);
        // 第 1 名的词全对说明不了什么，第 40000 名全错也一样
        assert!(all_right.theta <= THETA_MAX);
        assert!(all_wrong.theta >= THETA_MIN);
        assert!(all_right.theta.is_finite() && all_wrong.theta.is_finite());
    }

    #[test]
    fn 词汇量边界与能力互为反函数() {
        for rank in [100, 1500, 6000, 30000] {
            assert_eq!(vocabulary_rank(difficulty(rank)), rank);
        }
    }
}
