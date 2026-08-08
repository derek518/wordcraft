//! 预置蓝图。spec §4.2 F9「预置蓝图：小屋→城堡→村庄→城市」。
//!
//! **轮廓引导而非自动摆放**：点一下就铺好会把建造变成领奖。蓝图只在网格上
//! 投一层半透明轮廓，方块仍要用户自己放——它增强正在做的事，不替代它。
//!
//! ## 为什么是双层字符画
//!
//! spec 的四个阶段是一条**成长链**，不是四个互斥的选项。所以后一张图必须
//! 严格包含前一张：建小屋花的 24 块，在城堡里原样留着。
//!
//! 早先的写法是四张独立字符画，包含关系靠人工维护——结果实测 `hut → castle`
//! 只有 3/34 格能留下，四个阶段实际是四次推倒重来。四张图各画各的，
//! 谁也不会在改动时去核对另外三张。
//!
//! 现在拆成两层：一层写**方块类型**，一层写**它在第几阶段出现**。
//! 第 N 阶段 = 所有 `stage <= N` 的格子。包含关系成了结构性的，改图案时
//! 想破坏都破坏不掉。
//!
//! ## 画面
//!
//! 小屋在最高处，聚落从它脚下长出来——第一间屋子永远看得见，
//! 不会被后来的建筑埋掉。

use serde::Serialize;

/// 方块类型层。
///
/// - `#` 普通方块：墙体、地基这类占大头的结构
/// - `*` 稀有方块：塔尖等点睛处
/// - `@` 限定方块：纪念碑，全聚落最稀缺的位置
/// - 空格：该格不属于任何蓝图（门窗与留白）
const TYPES: &[&str] = &[
    " *       #       * ",
    " #      ###      # ",
    " #     #####     # ",
    " #     #####     # ",
    " #  *  #####  *  # ",
    " # ### ##### ### # ",
    " # ### ##### ### # ",
    "###################",
    "# ## ## #@# ## ## #",
    "###################",
    "###################",
    "##@## ###*### #####",
    "###################",
    "# # ## ##@## ## # #",
    "###################",
];

/// 阶段层。数字 = 该格在第几阶段出现，与 `TYPES` 逐格对应。
const STAGES: &[&str] = &[
    " 4       1       4 ",
    " 4      111      4 ",
    " 4     11111     4 ",
    " 4     11111     4 ",
    " 4  2  11111  2  4 ",
    " 4 222 11111 222 4 ",
    " 4 222 22222 222 4 ",
    "3332222222222222333",
    "3 32 22 222 22 23 3",
    "3332222222222222333",
    "3333333333333333333",
    "33333 3333333 33333",
    "4444444444444444444",
    "4 4 44 44444 44 4 4",
    "4444444444444444444",
];

/// 图案在 20×20 网格中的落点。上方留三行给塔尖的呼吸空间。
const ORIGIN: (i64, i64) = (0, 2);

/// (id, 名称, 描述)。描述**不写块数**——数量由图案算出，
/// 写死必然与图案漂移（改版前四条描述有三条对不上）。
const STAGE_META: [(&str, &str, &str); 4] = [
    ("hut", "小屋", "一间自己的木屋。它会一直是聚落的最高处。"),
    ("castle", "城堡", "在小屋脚下建起主楼与双塔，中央立起你的心。"),
    ("village", "村庄", "两侧住进邻人，中间辟出广场。"),
    ("city", "城市", "高塔刺入天际，街道向四方铺开。"),
];

fn block_of(ch: char) -> Option<&'static str> {
    match ch {
        '#' => Some("normal"),
        '*' => Some("rare"),
        '@' => Some("limited"),
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BlueprintCell {
    pub x: i64,
    pub y: i64,
    pub block_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Blueprint {
    pub id: String,
    pub name: String,
    pub description: String,
    /// 第几阶段，1 起。前端据此判断解锁顺序
    pub stage: i64,
    pub cells: Vec<BlueprintCell>,
    /// 各类型所需数量，前端据此提示「还差多少」
    pub required: Vec<(String, i64)>,
}

/// 逐格取出 (x, y, 类型, 阶段)。两层错位或写错字符都在这里暴露。
fn cells_with_stage() -> Vec<(i64, i64, &'static str, i64)> {
    let (ox, oy) = ORIGIN;
    let mut out = Vec::new();

    for (dy, (trow, srow)) in TYPES.iter().zip(STAGES.iter()).enumerate() {
        let tchars: Vec<char> = trow.chars().collect();
        let schars: Vec<char> = srow.chars().collect();

        for (dx, tch) in tchars.iter().enumerate() {
            let Some(block_type) = block_of(*tch) else { continue };
            // 类型层有格子而阶段层空白 = 两层没对齐，这块永远不会出现在任何蓝图里
            let stage = schars
                .get(dx)
                .and_then(|c| c.to_digit(10))
                .unwrap_or_else(|| panic!("({dx}, {dy}) 有方块 `{tch}` 但阶段层未标注"));

            out.push((ox + dx as i64, oy + dy as i64, block_type, stage as i64));
        }
    }
    out
}

pub fn all() -> Vec<Blueprint> {
    let cells = cells_with_stage();

    STAGE_META
        .iter()
        .enumerate()
        .map(|(i, (id, name, description))| {
            let stage = i as i64 + 1;
            // 累积：第 N 阶段包含此前所有阶段的格子
            let mine: Vec<BlueprintCell> = cells
                .iter()
                .filter(|(_, _, _, s)| *s <= stage)
                .map(|(x, y, t, _)| BlueprintCell {
                    x: *x,
                    y: *y,
                    block_type: (*t).to_string(),
                })
                .collect();

            let mut required: Vec<(String, i64)> = ["normal", "rare", "limited"]
                .iter()
                .map(|t| {
                    let n = mine.iter().filter(|c| c.block_type == *t).count() as i64;
                    ((*t).to_string(), n)
                })
                .filter(|(_, n)| *n > 0)
                .collect();
            required.sort();

            Blueprint {
                id: (*id).to_string(),
                name: (*name).to_string(),
                description: (*description).to_string(),
                stage,
                cells: mine,
                required,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::homestead::grants::MILESTONES;
    use std::collections::HashMap;

    const GRID: i64 = 20;

    fn need(bp: &Blueprint, block_type: &str) -> i64 {
        bp.required
            .iter()
            .find(|(t, _)| t == block_type)
            .map(|(_, n)| *n)
            .unwrap_or(0)
    }

    #[test]
    fn 两层字符画逐行等宽() {
        assert_eq!(TYPES.len(), STAGES.len(), "两层行数不一致");
        for (i, (t, s)) in TYPES.iter().zip(STAGES.iter()).enumerate() {
            assert_eq!(
                t.chars().count(),
                s.chars().count(),
                "第 {i} 行两层宽度不一致"
            );
        }
    }

    #[test]
    fn 后一张蓝图严格包含前一张() {
        // 这是整套设计的立足点：小屋花掉的方块，在城堡里必须原样留着。
        // 改版前四张独立字符画，hut→castle 只有 3/34 能留下，
        // 名义上的成长链实际是四次推倒重来
        let bps = all();
        for pair in bps.windows(2) {
            let (prev, next) = (&pair[0], &pair[1]);
            let later: HashMap<(i64, i64), &str> = next
                .cells
                .iter()
                .map(|c| ((c.x, c.y), c.block_type.as_str()))
                .collect();

            for c in &prev.cells {
                assert_eq!(
                    later.get(&(c.x, c.y)),
                    Some(&c.block_type.as_str()),
                    "{} 的 ({}, {}) 在 {} 中丢失或改型",
                    prev.id,
                    c.x,
                    c.y,
                    next.id
                );
            }
        }
    }

    #[test]
    fn 稀有方块需求不超过里程碑供给() {
        // 改版前小屋要 15 块稀有，而里程碑一共只发 5 块——第一个目标
        // 实际要几个月才够。没有任何测试拦住它
        let city = all().into_iter().last().unwrap();
        let demand = need(&city, "rare");
        assert!(
            demand <= MILESTONES.len() as i64,
            "最终蓝图需要 {demand} 块稀有方块，里程碑只发 {} 块",
            MILESTONES.len()
        );
    }

    #[test]
    fn 限定方块需求在合理的连续天数内() {
        // 限定方块每连续 7 天发一块。三块 = 21 天，一个学期内可达
        const MAX_LIMITED: i64 = 3;
        let city = all().into_iter().last().unwrap();
        let demand = need(&city, "limited");
        assert!(
            demand <= MAX_LIMITED,
            "最终蓝图需要 {demand} 块限定方块，即 {} 天连续打卡",
            demand * crate::homestead::grants::STREAK_STEP
        );
    }

    #[test]
    fn 小屋不依赖稀有与限定方块() {
        // 第一个目标必须只靠「继续答题」就能达成。卡在 200 词的里程碑
        // 或 7 天连续上，会让第一次成就感推迟到用户已经放弃之后
        let hut = &all()[0];
        assert_eq!(need(hut, "rare"), 0, "小屋不该需要稀有方块");
        assert_eq!(need(hut, "limited"), 0, "小屋不该需要限定方块");
        assert_eq!(
            need(hut, "normal"),
            hut.cells.len() as i64,
            "小屋应全部由普通方块构成"
        );
    }

    #[test]
    fn 小屋规模适合首个目标() {
        let hut = &all()[0];
        assert!(
            (15..=35).contains(&(hut.cells.len() as i64)),
            "小屋 {} 块：太小没有建造感，太大够不着",
            hut.cells.len()
        );
    }

    #[test]
    fn 四张蓝图规模递增且编号连续() {
        let bps = all();
        assert_eq!(bps.len(), 4);
        assert_eq!(
            bps.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(),
            vec!["hut", "castle", "village", "city"]
        );
        for (i, bp) in bps.iter().enumerate() {
            assert_eq!(bp.stage, i as i64 + 1);
        }
        let sizes: Vec<usize> = bps.iter().map(|b| b.cells.len()).collect();
        for pair in sizes.windows(2) {
            assert!(pair[0] < pair[1], "蓝图规模未递增: {sizes:?}");
        }
    }

    #[test]
    fn 全部格子落在网格内() {
        // 越界的格子永远画不出来，用户会看到一个残缺的轮廓却不知道为什么
        for bp in all() {
            for c in &bp.cells {
                assert!(
                    (0..GRID).contains(&c.x) && (0..GRID).contains(&c.y),
                    "{} 的格子 ({}, {}) 越界",
                    bp.id,
                    c.x,
                    c.y
                );
            }
        }
    }

    #[test]
    fn 同一坐标不重复出现() {
        for bp in all() {
            let mut seen = std::collections::HashSet::new();
            for c in &bp.cells {
                assert!(
                    seen.insert((c.x, c.y)),
                    "{} 的坐标 ({}, {}) 重复",
                    bp.id,
                    c.x,
                    c.y
                );
            }
        }
    }

    #[test]
    fn 所需数量与格子数一致() {
        // 前端靠 required 提示「还差多少块」，对不上会误导用户
        for bp in all() {
            let total: i64 = bp.required.iter().map(|(_, n)| n).sum();
            assert_eq!(total, bp.cells.len() as i64, "{} 的数量统计不符", bp.id);
        }
    }

    #[test]
    fn 描述不含块数() {
        // 早先描述写死「28 块」而图案实际 34 块，四条错了三条。
        // 数量只能有一个来源：图案本身
        for bp in all() {
            assert!(
                !bp.description.contains('块') || !bp.description.chars().any(|c| c.is_ascii_digit()),
                "{} 的描述写了块数，会与图案漂移: {}",
                bp.id,
                bp.description
            );
        }
    }

    #[test]
    fn 城市规模填满网格的合理比例() {
        let city = all().into_iter().last().unwrap();
        let ratio = city.cells.len() as f64 / (GRID * GRID) as f64;
        // 太稀会让网格显得空旷无从下手，太密则没有留白
        assert!(
            (0.40..=0.70).contains(&ratio),
            "城市占网格 {:.0}%，不在 40%–70% 区间",
            ratio * 100.0
        );
    }

    #[test]
    fn 空格与点号都不产生格子() {
        assert!(block_of(' ').is_none());
        assert!(block_of('.').is_none());
        assert_eq!(block_of('#'), Some("normal"));
        assert_eq!(block_of('*'), Some("rare"));
        assert_eq!(block_of('@'), Some("limited"));
    }
}
