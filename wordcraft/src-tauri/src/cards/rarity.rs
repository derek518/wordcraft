//! 稀有度抽取。contracts §10.2。
//!
//! 纯逻辑，可穷举验证分布——概率写错不会报错，只会让传说卡永远不出或者
//! 遍地都是，而这种偏差要抽上千次才会被人察觉。

/// 稀有度权重，对应契约的 70% / 25% / 5%。
///
/// 用整数权重而非浮点概率：浮点累加会有舍入误差，整数总和恒为 100，
/// 「加起来是不是 100」一眼可验。
pub const WEIGHTS: [(i64, u32); 3] = [(1, 70), (2, 25), (3, 5)];

/// 按权重挑选稀有度。`roll` 取 [0, 100) 区间。
pub fn pick_rarity(roll: u32) -> i64 {
    let mut acc = 0;
    for (rarity, weight) in WEIGHTS {
        acc += weight;
        if roll < acc {
            return rarity;
        }
    }
    // 权重总和为 100 且 roll < 100 时不可达。
    // 兜底给普通卡而非 panic——抽卡失败不该让整个应用崩溃
    WEIGHTS[0].0
}

/// 线性同余伪随机数。
///
/// 抽卡不需要密码学强度，但需要**可播种**——测试要能复现特定序列，
/// 否则概率分布的断言只能靠大数定律碰运气。
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        // 种子为 0 时 LCG 会退化，换一个非零值
        Self(if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed })
    }

    /// 从系统时间播种。
    pub fn from_clock() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64 ^ d.as_secs())
            .unwrap_or(0x5EED);
        Self::new(nanos)
    }

    pub fn next_u32(&mut self, bound: u32) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as u32) % bound.max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 权重总和为一百() {
        let total: u32 = WEIGHTS.iter().map(|(_, w)| w).sum();
        assert_eq!(total, 100, "权重总和不是 100，概率会失真");
    }

    #[test]
    fn 边界值落在正确的稀有度() {
        // 0..69 → 普通，70..94 → 稀有，95..99 → 传说
        assert_eq!(pick_rarity(0), 1);
        assert_eq!(pick_rarity(69), 1);
        assert_eq!(pick_rarity(70), 2);
        assert_eq!(pick_rarity(94), 2);
        assert_eq!(pick_rarity(95), 3);
        assert_eq!(pick_rarity(99), 3);
    }

    #[test]
    fn 越界输入不会崩溃() {
        // 抽卡失败不该让应用崩溃
        assert_eq!(pick_rarity(100), 1);
        assert_eq!(pick_rarity(u32::MAX), 1);
    }

    #[test]
    fn 实际分布接近设计值() {
        let mut rng = Rng::new(0xC0FFEE);
        let mut counts = [0usize; 4];
        const N: usize = 100_000;

        for _ in 0..N {
            counts[pick_rarity(rng.next_u32(100)) as usize] += 1;
        }

        let pct = |i: usize| counts[i] as f64 / N as f64 * 100.0;
        // 10 万次采样，±2% 容差足够宽松又能捕捉到真实偏差
        assert!((pct(1) - 70.0).abs() < 2.0, "普通卡占比 {:.1}%", pct(1));
        assert!((pct(2) - 25.0).abs() < 2.0, "稀有卡占比 {:.1}%", pct(2));
        assert!((pct(3) - 5.0).abs() < 2.0, "传说卡占比 {:.1}%", pct(3));
    }

    #[test]
    fn 同种子产出同序列() {
        // 可复现是测试概率逻辑的前提
        let seq = |seed| {
            let mut r = Rng::new(seed);
            (0..20).map(|_| r.next_u32(100)).collect::<Vec<_>>()
        };
        assert_eq!(seq(42), seq(42));
        assert_ne!(seq(42), seq(43));
    }

    #[test]
    fn 零种子不退化() {
        let mut r = Rng::new(0);
        let values: Vec<u32> = (0..10).map(|_| r.next_u32(100)).collect();
        assert!(
            values.iter().any(|v| *v != values[0]),
            "零种子导致序列退化为常量: {values:?}"
        );
    }

    #[test]
    fn 上界为零不会崩溃() {
        let mut r = Rng::new(1);
        assert_eq!(r.next_u32(0), 0);
    }
}
