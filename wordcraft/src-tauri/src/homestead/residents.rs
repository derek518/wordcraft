//! 蓝图完成判定与入住位规则。
//!
//! 改版前完成一张蓝图什么都不会发生——进度条走到 100%，然后没有了。
//! 这是家园最大的空洞：建造没有结果，也就没有回来的理由。
//!
//! 现在完成蓝图解锁入住位，收集到的生物可以住进来。抽卡第一次有了用处，
//! 家园第一次有了活物。两个系统此前各自孤立。
//!
//! 纯逻辑，不碰数据库。

use super::blueprints::Blueprint;
use crate::db::repo::homestead::PlacedBlock;
use std::collections::HashMap;

/// 每完成一张蓝图新增的入住位。索引对应阶段 1..4。
///
/// 递增而非等量：小屋只住得下一个，城市该热闹些。
/// 累计 6 位，而生物卡池共 16 张——位置始终稀缺，
/// 让「让谁住进来」保持是个选择而不是照单全收。
const SLOTS_PER_STAGE: [i64; 4] = [1, 1, 2, 2];

/// 已建成的蓝图 id，按阶段顺序。
///
/// 「建成」= 蓝图要求的每一格都放着**对应类型**的方块。类型放错不算——
/// 否则用普通方块糊满就能骗过塔尖的稀有位。
pub fn completed(grid: &[PlacedBlock], blueprints: &[Blueprint]) -> Vec<String> {
    let placed: HashMap<(i64, i64), &str> = grid
        .iter()
        .map(|b| ((b.x, b.y), b.block_type.as_str()))
        .collect();

    blueprints
        .iter()
        .filter(|bp| {
            bp.cells
                .iter()
                .all(|c| placed.get(&(c.x, c.y)) == Some(&c.block_type.as_str()))
        })
        .map(|bp| bp.id.clone())
        .collect()
}

/// 已解锁的入住位总数。
pub fn slots_for(completed_count: usize) -> i64 {
    SLOTS_PER_STAGE.iter().take(completed_count).sum()
}

/// 全部建成时的入住位上限。
pub fn max_slots() -> i64 {
    SLOTS_PER_STAGE.iter().sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::homestead::blueprints;

    fn block(x: i64, y: i64, t: &str) -> PlacedBlock {
        PlacedBlock {
            x,
            y,
            block_type: t.to_string(),
        }
    }

    /// 把某张蓝图完整摆出来。
    fn build(id: &str) -> Vec<PlacedBlock> {
        blueprints::all()
            .into_iter()
            .find(|b| b.id == id)
            .unwrap()
            .cells
            .into_iter()
            .map(|c| block(c.x, c.y, &c.block_type))
            .collect()
    }

    #[test]
    fn 空网格没有任何蓝图完成() {
        assert!(completed(&[], &blueprints::all()).is_empty());
    }

    #[test]
    fn 摆满小屋即判定完成() {
        let done = completed(&build("hut"), &blueprints::all());
        assert_eq!(done, vec!["hut"]);
    }

    #[test]
    fn 完成城堡时小屋同时成立() {
        // 扩建链的直接后果：建成城堡意味着小屋的每一块都还在原位。
        // 这正是双层字符画要保证的事，在这里被再验证一次
        let done = completed(&build("castle"), &blueprints::all());
        assert_eq!(done, vec!["hut", "castle"]);
    }

    #[test]
    fn 建成城市时四张蓝图全部成立() {
        let done = completed(&build("city"), &blueprints::all());
        assert_eq!(done, vec!["hut", "castle", "village", "city"]);
    }

    #[test]
    fn 少一块就不算完成() {
        let mut grid = build("hut");
        grid.pop();
        assert!(completed(&grid, &blueprints::all()).is_empty());
    }

    #[test]
    fn 类型放错不算完成() {
        // 塔尖要稀有方块。允许普通方块顶替，稀缺资源就失去意义了
        let bps = blueprints::all();
        let castle = bps.iter().find(|b| b.id == "castle").unwrap();
        let spire = castle.cells.iter().find(|c| c.block_type == "rare").unwrap();

        let grid: Vec<PlacedBlock> = castle
            .cells
            .iter()
            .map(|c| {
                let t = if (c.x, c.y) == (spire.x, spire.y) {
                    "normal" // 用普通方块冒充稀有塔尖
                } else {
                    c.block_type.as_str()
                };
                block(c.x, c.y, t)
            })
            .collect();

        assert!(
            !completed(&grid, &bps).contains(&"castle".to_string()),
            "普通方块顶替稀有位不该判定为完成"
        );
    }

    #[test]
    fn 多放的方块不影响判定() {
        // 用户可以在蓝图之外自由建造，那不该妨碍蓝图达成
        let mut grid = build("hut");
        grid.push(block(19, 19, "normal"));
        assert_eq!(completed(&grid, &blueprints::all()), vec!["hut"]);
    }

    #[test]
    fn 入住位随完成数递增() {
        assert_eq!(slots_for(0), 0, "一张都没建成时不该有居民");
        assert_eq!(slots_for(1), 1);
        assert_eq!(slots_for(2), 2);
        assert_eq!(slots_for(3), 4);
        assert_eq!(slots_for(4), 6);
    }

    #[test]
    fn 入住位不超过上限() {
        // 传入超出蓝图数的值不能算出更多位置
        assert_eq!(slots_for(99), max_slots());
        assert_eq!(max_slots(), 6);
    }

    #[test]
    fn 入住位始终少于生物卡池() {
        // 位置比生物少，「让谁住进来」才是个选择。
        // 卡池 16 张生物（见 004_card_pool.sql）
        const CREATURE_CARDS: i64 = 16;
        assert!(
            max_slots() < CREATURE_CARDS,
            "入住位 {} 已不少于生物总数 {CREATURE_CARDS}，选择消失",
            max_slots()
        );
    }
}
