//! 预置蓝图。spec §4.2 F9「预置蓝图：小屋→城堡→村庄→城市」。
//!
//! **轮廓引导而非自动摆放**：点一下就铺好会把建造变成领奖。蓝图只在网格上
//! 投一层半透明轮廓，方块仍要用户自己放——它增强正在做的事，不替代它。
//!
//! 图案用字符画定义。手写坐标数组既看不出形状，改一个格子还要重排后面所有
//! 索引；字符画则是所见即所得。

use serde::Serialize;

/// 字符 → 方块类型。空格与 `.` 表示该格不属于蓝图。
///
/// - `#` 普通方块：墙体、地基这类占大头的结构
/// - `*` 稀有方块：塔尖、装饰等点睛处
/// - `@` 限定方块：全图唯一的核心，用最稀缺的方块
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
    pub cells: Vec<BlueprintCell>,
    /// 各类型所需数量，前端据此提示「还差多少」
    pub required: Vec<(String, i64)>,
}

/// (id, 名称, 描述, 左上角落点, 图案)
///
/// 落点让图案在 20×20 内居中偏上，底部留白给未来的装饰。
type Spec = (&'static str, &'static str, &'static str, (i64, i64), &'static [&'static str]);

const SPECS: &[Spec] = &[
    (
        "hut",
        "小屋",
        "一间遮风挡雨的木屋。28 块，约五天。",
        (7, 6),
        &[
            "  ***  ",
            " ***** ",
            "*******",
            "#######",
            "#.....#",
            "#..@..#",
            "#######",
        ],
    ),
    (
        "castle",
        "城堡",
        "带塔楼的石制城堡。47 块，约八天。",
        (4, 4),
        &[
            "*.........*",
            "*.........*",
            "#*.......*#",
            "###########",
            "#.#.....#.#",
            "#.#..@..#.#",
            "#.#.....#.#",
            "#.........#",
            "#..#...#..#",
            "###########",
        ],
    ),
    (
        "village",
        "村庄",
        "几户人家与中央广场。100 块，约半个月。",
        (0, 3),
        &[
            " ***    ***    *** ",
            "#####  #####  #####",
            "#...#  #...#  #...#",
            "#####  #####  #####",
            "                   ",
            "     #########     ",
            "     #.......#     ",
            "     #...@...#     ",
            "     #.......#     ",
            "     #########     ",
            "                   ",
            "#####  #####  #####",
            "#...#  #...#  #...#",
            "#####  #####  #####",
        ],
    ),
    (
        "city",
        "城市",
        "高塔林立的都市。142 块，约三周。",
        (1, 1),
        &[
            "  *     *       *  ",
            " ***   ***     *** ",
            " #.#   #.#     #.# ",
            " #.#   #.#  *  #.# ",
            " #.#   #.# *** #.# ",
            " #.#   #.# #.# #.# ",
            "###########.#######",
            "#.................#",
            "#.###.###.###.###.#",
            "#.#.....@.......#.#",
            "#.###.###.###.###.#",
            "#.................#",
            "###################",
            "#..#..#..#..#..#..#",
            "###################",
        ],
    ),
];

fn parse(spec: &Spec) -> Blueprint {
    let (id, name, description, (ox, oy), rows) = spec;
    let mut cells = Vec::new();

    for (dy, row) in rows.iter().enumerate() {
        for (dx, ch) in row.chars().enumerate() {
            if let Some(block_type) = block_of(ch) {
                cells.push(BlueprintCell {
                    x: ox + dx as i64,
                    y: oy + dy as i64,
                    block_type: block_type.to_string(),
                });
            }
        }
    }

    let mut required: Vec<(String, i64)> = ["normal", "rare", "limited"]
        .iter()
        .map(|t| {
            let n = cells.iter().filter(|c| c.block_type == *t).count() as i64;
            ((*t).to_string(), n)
        })
        .filter(|(_, n)| *n > 0)
        .collect();
    required.sort();

    Blueprint {
        id: (*id).to_string(),
        name: (*name).to_string(),
        description: (*description).to_string(),
        cells,
        required,
    }
}

pub fn all() -> Vec<Blueprint> {
    SPECS.iter().map(parse).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GRID: i64 = 20;

    #[test]
    fn 四张蓝图按规模递增() {
        let bps = all();
        assert_eq!(bps.len(), 4);
        assert_eq!(
            bps.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(),
            vec!["hut", "castle", "village", "city"]
        );

        // spec 定的顺序是小屋→城堡→村庄→城市，规模必须真的递增，
        // 否则「进阶」的说法不成立
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
    fn 每张蓝图都有唯一的限定方块核心() {
        // 限定方块最稀缺（7 天连续打卡才 1 块），每图放一个作为核心，
        // 多了会让蓝图无法完成
        for bp in all() {
            let n = bp
                .cells
                .iter()
                .filter(|c| c.block_type == "limited")
                .count();
            assert_eq!(n, 1, "{} 的限定方块数应为 1，实际 {n}", bp.id);
        }
    }

    #[test]
    fn 小屋规模适合首个目标() {
        // 首张蓝图要在几天内可达：按每天 6 个新词算，30 块约五天。
        // 门槛过高会让蓝图从目标变成劝退
        let hut = &all()[0];
        assert!(
            hut.cells.len() <= 35,
            "小屋 {} 块，作为第一个目标偏大",
            hut.cells.len()
        );
    }

    #[test]
    fn 城市规模不超出网格容量() {
        let city = all().into_iter().last().unwrap();
        assert!(
            city.cells.len() < (GRID * GRID) as usize,
            "城市 {} 块超出 400 格容量",
            city.cells.len()
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
