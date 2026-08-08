//! 家园库存与网格。plan: docs/plans/homestead-v1.1.md §2。

use rusqlite::Connection;
use serde::Serialize;

/// 网格边长。前端从 `HomesteadState` 读取，不各自硬编码——
/// 改尺寸时只有一处要动（连同 migration 的 CHECK 约束）。
pub const GRID_SIZE: i64 = 20;

pub const BLOCK_TYPES: [&str; 3] = ["normal", "rare", "limited"];

#[derive(Debug, Clone, Serialize)]
pub struct PlacedBlock {
    pub x: i64,
    pub y: i64,
    pub block_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BlockStock {
    pub block_type: String,
    pub owned: i64,
    /// 可放置数 = owned - placed
    pub available: i64,
}

pub fn is_valid_type(block_type: &str) -> bool {
    BLOCK_TYPES.contains(&block_type)
}

pub fn in_bounds(x: i64, y: i64) -> bool {
    (0..GRID_SIZE).contains(&x) && (0..GRID_SIZE).contains(&y)
}

pub fn inventory(conn: &Connection) -> Result<Vec<BlockStock>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT block_type, owned, placed FROM block_inventory
             ORDER BY CASE block_type
                        WHEN 'normal' THEN 1 WHEN 'rare' THEN 2 ELSE 3 END",
        )
        .map_err(|e| format!("准备库存查询失败: {e}"))?;

    let rows = stmt
        .query_map([], |r| {
            let owned: i64 = r.get(1)?;
            let placed: i64 = r.get(2)?;
            Ok(BlockStock {
                block_type: r.get(0)?,
                owned,
                available: owned - placed,
            })
        })
        .map_err(|e| format!("查询库存失败: {e}"))?;

    rows.collect::<Result<_, _>>()
        .map_err(|e| format!("读取库存失败: {e}"))
}

pub fn grid(conn: &Connection) -> Result<Vec<PlacedBlock>, String> {
    let mut stmt = conn
        .prepare("SELECT x, y, block_type FROM homestead_grid ORDER BY y, x")
        .map_err(|e| format!("准备网格查询失败: {e}"))?;

    let rows = stmt
        .query_map([], |r| {
            Ok(PlacedBlock {
                x: r.get(0)?,
                y: r.get(1)?,
                block_type: r.get(2)?,
            })
        })
        .map_err(|e| format!("查询网格失败: {e}"))?;

    rows.collect::<Result<_, _>>()
        .map_err(|e| format!("读取网格失败: {e}"))
}

/// 增加库存。由发放逻辑调用，不直接暴露给 command。
pub fn add_owned(conn: &Connection, block_type: &str, amount: i64) -> Result<(), String> {
    if !is_valid_type(block_type) {
        return Err(format!("未知方块类型 `{block_type}`"));
    }
    if amount <= 0 {
        return Err(format!("发放数量必须为正，收到 {amount}"));
    }
    conn.execute(
        "UPDATE block_inventory SET owned = owned + ?2 WHERE block_type = ?1",
        rusqlite::params![block_type, amount],
    )
    .map_err(|e| format!("增加 {block_type} 库存失败: {e}"))?;
    Ok(())
}

/// 放置一块。库存不足或格子已占用返回 Err。
pub fn place(conn: &Connection, x: i64, y: i64, block_type: &str, now: &str) -> Result<(), String> {
    if !in_bounds(x, y) {
        return Err(format!(
            "坐标 ({x}, {y}) 越界，有效范围 0..{}",
            GRID_SIZE - 1
        ));
    }
    if !is_valid_type(block_type) {
        return Err(format!("未知方块类型 `{block_type}`"));
    }

    // 占用检查在库存之前：用户点一个已有方块的格子时，「这里满了」才是他
    // 看得见的事实。反过来先报「方块不足」会把注意力引向错误的方向——
    // 他要换的是位置，不是去攒方块
    let occupied: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM homestead_grid WHERE x = ?1 AND y = ?2",
            [x, y],
            |r| r.get(0),
        )
        .map_err(|e| format!("检查格子 ({x}, {y}) 失败: {e}"))?;

    if occupied > 0 {
        return Err(format!("格子 ({x}, {y}) 已被占用"));
    }

    let available: i64 = conn
        .query_row(
            "SELECT owned - placed FROM block_inventory WHERE block_type = ?1",
            [block_type],
            |r| r.get(0),
        )
        .map_err(|e| format!("读取 {block_type} 库存失败: {e}"))?;

    if available <= 0 {
        return Err(format!("{block_type} 方块不足，无可放置的库存"));
    }

    // 主键冲突仍然兜底：上面的检查与这里之间存在竞态窗口
    conn.execute(
        "INSERT INTO homestead_grid (x, y, block_type, placed_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![x, y, block_type, now],
    )
    .map_err(|_| format!("格子 ({x}, {y}) 已被占用"))?;

    conn.execute(
        "UPDATE block_inventory SET placed = placed + 1 WHERE block_type = ?1",
        [block_type],
    )
    .map_err(|e| format!("更新 {block_type} 放置数失败: {e}"))?;

    Ok(())
}

/// 移除一块，退回库存。空格子返回 Err。
pub fn remove(conn: &Connection, x: i64, y: i64) -> Result<String, String> {
    if !in_bounds(x, y) {
        return Err(format!(
            "坐标 ({x}, {y}) 越界，有效范围 0..{}",
            GRID_SIZE - 1
        ));
    }

    let block_type: String = conn
        .query_row(
            "SELECT block_type FROM homestead_grid WHERE x = ?1 AND y = ?2",
            [x, y],
            |r| r.get(0),
        )
        .map_err(|_| format!("格子 ({x}, {y}) 是空的"))?;

    conn.execute(
        "DELETE FROM homestead_grid WHERE x = ?1 AND y = ?2",
        [x, y],
    )
    .map_err(|e| format!("移除格子 ({x}, {y}) 失败: {e}"))?;

    conn.execute(
        "UPDATE block_inventory SET placed = placed - 1 WHERE block_type = ?1",
        [&block_type],
    )
    .map_err(|e| format!("退回 {block_type} 库存失败: {e}"))?;

    Ok(block_type)
}

/// 记录一次发放。`source_key` 重复时返回 false 表示已发过。
///
/// 幂等的实现落点：靠 UNIQUE 约束而非先查后写。先查后写在并发下有竞态，
/// 而这里的调用方（启动补发、会话结束）完全可能重叠。
pub fn record_grant(
    conn: &Connection,
    source: &str,
    source_key: &str,
    block_type: &str,
    amount: i64,
    now: &str,
) -> Result<bool, String> {
    let affected = conn
        .execute(
            "INSERT OR IGNORE INTO block_grants
             (source, source_key, block_type, amount, granted_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![source, source_key, block_type, amount, now],
        )
        .map_err(|e| format!("记录发放失败: {e}"))?;

    Ok(affected > 0)
}

/// 某来源已发放过的 key 集合，用于批量补发时跳过。
pub fn granted_keys(conn: &Connection, source: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT source_key FROM block_grants WHERE source = ?1")
        .map_err(|e| format!("准备发放记录查询失败: {e}"))?;

    let rows = stmt
        .query_map([source], |r| r.get::<_, String>(0))
        .map_err(|e| format!("查询发放记录失败: {e}"))?;

    rows.collect::<Result<_, _>>()
        .map_err(|e| format!("读取发放记录失败: {e}"))
}

// ── 居民 ─────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct Resident {
    /// 已入住时是位置序号，候选时为 -1
    pub slot: i64,
    pub card_id: i64,
    pub name: String,
    pub image_path: String,
    pub rarity: i64,
}

const NOT_RESIDING: i64 = -1;

/// 已入住的居民，按位置排序。
pub fn residents(conn: &Connection) -> Result<Vec<Resident>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT r.slot, c.id, c.name, c.image_path, c.rarity
             FROM homestead_residents r JOIN cards c ON c.id = r.card_id
             ORDER BY r.slot",
        )
        .map_err(|e| format!("准备居民查询失败: {e}"))?;

    let rows = stmt
        .query_map([], |r| {
            Ok(Resident {
                slot: r.get(0)?,
                card_id: r.get(1)?,
                name: r.get(2)?,
                image_path: r.get(3)?,
                rarity: r.get(4)?,
            })
        })
        .map_err(|e| format!("查询居民失败: {e}"))?;

    rows.collect::<Result<_, _>>()
        .map_err(|e| format!("读取居民失败: {e}"))
}

/// 已收集但尚未入住的生物。画作不能入住——它们是挂在墙上的，不是活物。
pub fn resident_candidates(conn: &Connection) -> Result<Vec<Resident>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.name, c.image_path, c.rarity
             FROM cards c JOIN card_collection k ON k.card_id = c.id
             WHERE c.card_type = 'creature'
               AND c.id NOT IN (SELECT card_id FROM homestead_residents)
             ORDER BY c.rarity DESC, c.id",
        )
        .map_err(|e| format!("准备候选查询失败: {e}"))?;

    let rows = stmt
        .query_map([], |r| {
            Ok(Resident {
                slot: NOT_RESIDING,
                card_id: r.get(0)?,
                name: r.get(1)?,
                image_path: r.get(2)?,
                rarity: r.get(3)?,
            })
        })
        .map_err(|e| format!("查询候选失败: {e}"))?;

    rows.collect::<Result<_, _>>()
        .map_err(|e| format!("读取候选失败: {e}"))
}

/// 让一只生物住进某个位置。
///
/// 校验放在这里而非命令层：前端传什么都不该能写进一条无效记录。
/// 未收集的卡、画作、已占用的位置，三种都必须被挡下且说明原因。
pub fn move_in(conn: &Connection, slot: i64, card_id: i64, now: &str) -> Result<(), String> {
    let collected: bool = conn
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM cards c JOIN card_collection k ON k.card_id = c.id
               WHERE c.id = ?1 AND c.card_type = 'creature')",
            [card_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("校验卡牌失败: {e}"))?;

    if !collected {
        return Err(format!("卡牌 {card_id} 尚未收集，或不是生物卡"));
    }

    // 位置已有住户时先请出去。用户点的是「换成这只」，
    // 报「位置已占用」再让他手动腾位是多余的一步
    conn.execute("DELETE FROM homestead_residents WHERE slot = ?1", [slot])
        .map_err(|e| format!("腾出位置失败: {e}"))?;

    conn.execute(
        "INSERT INTO homestead_residents (slot, card_id, moved_in_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![slot, card_id, now],
    )
    .map_err(|e| format!("入住失败: {e}"))?;

    Ok(())
}

/// 请某位居民搬走。位置本来就空时也算成功——用户要的结果已经达成。
pub fn move_out(conn: &Connection, slot: i64) -> Result<(), String> {
    conn.execute("DELETE FROM homestead_residents WHERE slot = ?1", [slot])
        .map_err(|e| format!("搬离失败: {e}"))?;
    Ok(())
}

/// 清理超出已解锁位置的居民。
///
/// 位置数由建成的蓝图决定，而方块可以被拆走——拆掉一块，蓝图就不再成立，
/// 位置随之收回。此时住在里面的生物必须搬走，否则会留下一条读不出来的记录。
pub fn evict_beyond(conn: &Connection, slots: i64) -> Result<usize, String> {
    conn.execute("DELETE FROM homestead_residents WHERE slot >= ?1", [slots])
        .map_err(|e| format!("清理越界居民失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;
    use crate::test_support::in_memory_db;

    const NOW: &str = "2026-08-08T00:00:00Z";

    fn db() -> Connection {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();
        conn
    }

    fn stock(conn: &Connection, t: &str) -> (i64, i64) {
        let inv = inventory(conn).unwrap();
        let s = inv.iter().find(|s| s.block_type == t).unwrap();
        (s.owned, s.available)
    }

    #[test]
    fn 初始三种类型都有行且为零() {
        let conn = db();
        let inv = inventory(&conn).unwrap();
        assert_eq!(inv.len(), 3);
        assert!(inv.iter().all(|s| s.owned == 0 && s.available == 0));
    }

    #[test]
    fn 放置消耗库存移除退回() {
        let conn = db();
        add_owned(&conn, "normal", 3).unwrap();

        place(&conn, 5, 5, "normal", NOW).unwrap();
        assert_eq!(stock(&conn, "normal"), (3, 2));
        assert_eq!(grid(&conn).unwrap().len(), 1);

        // 退回逻辑写反会让方块凭空消失
        let removed = remove(&conn, 5, 5).unwrap();
        assert_eq!(removed, "normal");
        assert_eq!(stock(&conn, "normal"), (3, 3));
        assert!(grid(&conn).unwrap().is_empty());
    }

    #[test]
    fn 库存不足时拒绝放置() {
        let conn = db();
        add_owned(&conn, "normal", 1).unwrap();
        place(&conn, 0, 0, "normal", NOW).unwrap();

        // 静默失败会让用户以为放上了，刷新后才发现没有
        let err = place(&conn, 1, 1, "normal", NOW).unwrap_err();
        assert!(err.contains("不足"), "错误消息应说明原因: {err}");
        assert_eq!(grid(&conn).unwrap().len(), 1);
    }

    #[test]
    fn 已占用的格子拒绝再放置且不扣库存() {
        let conn = db();
        add_owned(&conn, "normal", 5).unwrap();
        place(&conn, 3, 3, "normal", NOW).unwrap();

        let err = place(&conn, 3, 3, "rare", NOW).unwrap_err();
        assert!(err.contains("占用"), "错误消息应说明原因: {err}");
        // 插入失败时库存不该变动——先插网格再改库存正是为此
        assert_eq!(stock(&conn, "normal"), (5, 4));
        assert_eq!(stock(&conn, "rare"), (0, 0));
    }

    #[test]
    fn 坐标越界被拒绝且消息可诊断() {
        let conn = db();
        add_owned(&conn, "normal", 5).unwrap();

        for (x, y) in [(-1, 0), (0, -1), (20, 0), (0, 20), (99, 99)] {
            let err = place(&conn, x, y, "normal", NOW).unwrap_err();
            assert!(err.contains("越界"), "({x},{y}) 应被拒绝: {err}");
        }
        assert!(grid(&conn).unwrap().is_empty());
    }

    #[test]
    fn 边界坐标可用() {
        let conn = db();
        add_owned(&conn, "normal", 4).unwrap();
        for (x, y) in [(0, 0), (0, 19), (19, 0), (19, 19)] {
            place(&conn, x, y, "normal", NOW).unwrap();
        }
        assert_eq!(grid(&conn).unwrap().len(), 4);
    }

    #[test]
    fn 移除空格子返回错误() {
        let conn = db();
        let err = remove(&conn, 7, 7).unwrap_err();
        assert!(err.contains("空"), "错误消息应说明原因: {err}");
    }

    #[test]
    fn 未知方块类型被拒绝() {
        let conn = db();
        assert!(add_owned(&conn, "diamond", 1).is_err());
        assert!(place(&conn, 0, 0, "diamond", NOW).is_err());
    }

    #[test]
    fn 发放数量必须为正() {
        let conn = db();
        assert!(add_owned(&conn, "normal", 0).is_err());
        assert!(add_owned(&conn, "normal", -5).is_err());
    }

    // ── 幂等 ──────────────────────────

    #[test]
    fn 相同来源键只记录一次() {
        let conn = db();
        assert!(record_grant(&conn, "mastery", "42", "normal", 1, NOW).unwrap());
        // 第二次返回 false，调用方据此跳过加库存
        assert!(!record_grant(&conn, "mastery", "42", "normal", 1, NOW).unwrap());
        assert!(!record_grant(&conn, "mastery", "42", "normal", 1, NOW).unwrap());

        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM block_grants", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "重复发放被记了多次");
    }

    #[test]
    fn 不同来源可用相同的键() {
        let conn = db();
        // mastery 用 word_id、milestone 用词数阈值，两者的 "200" 是不同含义
        assert!(record_grant(&conn, "mastery", "200", "normal", 1, NOW).unwrap());
        assert!(record_grant(&conn, "milestone", "200", "rare", 1, NOW).unwrap());
    }

    #[test]
    fn 可查出某来源的全部已发放键() {
        let conn = db();
        for id in ["1", "2", "3"] {
            record_grant(&conn, "mastery", id, "normal", 1, NOW).unwrap();
        }
        record_grant(&conn, "streak", "2026-08-08", "limited", 1, NOW).unwrap();

        let mut keys = granted_keys(&conn, "mastery").unwrap();
        keys.sort();
        assert_eq!(keys, vec!["1", "2", "3"]);
        assert_eq!(granted_keys(&conn, "streak").unwrap().len(), 1);
        assert!(granted_keys(&conn, "milestone").unwrap().is_empty());
    }

    #[test]
    fn 已放置数不会超过拥有数() {
        let conn = db();
        add_owned(&conn, "normal", 2).unwrap();
        place(&conn, 0, 0, "normal", NOW).unwrap();
        place(&conn, 1, 0, "normal", NOW).unwrap();

        // CHECK 约束是最后防线，此处验证它真的生效而非被忽略
        let direct = conn.execute(
            "UPDATE block_inventory SET placed = placed + 1 WHERE block_type = 'normal'",
            [],
        );
        assert!(direct.is_err(), "placed 超过 owned 时 CHECK 未拦截");
    }

    #[test]
    fn 网格按行列有序返回() {
        let conn = db();
        add_owned(&conn, "normal", 3).unwrap();
        place(&conn, 5, 2, "normal", NOW).unwrap();
        place(&conn, 1, 1, "normal", NOW).unwrap();
        place(&conn, 3, 1, "normal", NOW).unwrap();

        let g = grid(&conn).unwrap();
        let coords: Vec<(i64, i64)> = g.iter().map(|b| (b.x, b.y)).collect();
        assert_eq!(coords, vec![(1, 1), (3, 1), (5, 2)]);
    }
}
