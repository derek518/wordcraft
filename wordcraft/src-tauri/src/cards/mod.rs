//! 抽卡与图鉴。contracts §10。
//!
//! 决议 S9 把抽卡提前进 MVP：长期钩子（家园/赛道）全在 P1，MVP 只剩
//! streak + XP，而 streak 本身有 S1/S4/S6 三处缺陷。抽卡实现成本远低于
//! 家园建造，即时奖励的心理效果却更强。

mod rarity;

pub use rarity::{pick_rarity, Rng};

use crate::db::{clock, Db};
use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct Card {
    pub id: i64,
    pub name: String,
    pub card_type: String,
    pub rarity: i64,
    pub image_path: String,
    pub trivia: String,
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct DrawResult {
    pub card: Card,
    /// 是否首次获得——决定开卡动画放「新卡」还是「重复」
    pub is_first: bool,
    /// 该卡累计张数
    pub count: i64,
    pub tickets_left: i64,
}

#[derive(Debug, Serialize)]
pub struct CollectionEntry {
    pub card: Card,
    /// 0 表示未收集，图鉴显示剪影
    pub count: i64,
    pub is_new: bool,
    pub first_at: Option<String>,
}

fn row_to_card(row: &rusqlite::Row) -> rusqlite::Result<Card> {
    Ok(Card {
        id: row.get("id")?,
        name: row.get("name")?,
        card_type: row.get("card_type")?,
        rarity: row.get("rarity")?,
        image_path: row.get("image_path")?,
        trivia: row.get("trivia")?,
        source: row.get("source")?,
    })
}

/// 在指定稀有度中随机取一张。
///
/// 该稀有度无卡时降级到普通卡：卡池尚未补齐时，抽卡不该因为「没有传说卡」
/// 而报错——用户付出的券是真的，必须给回东西。
fn pick_card(conn: &Connection, rarity: i64, rng: &mut Rng) -> Result<Card, String> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cards WHERE rarity = ?1",
            [rarity],
            |r| r.get(0),
        )
        .map_err(|e| format!("统计稀有度 {rarity} 卡数失败: {e}"))?;

    let (target_rarity, count) = if count > 0 {
        (rarity, count)
    } else {
        let fallback: i64 = conn
            .query_row("SELECT COUNT(*) FROM cards WHERE rarity = 1", [], |r| {
                r.get(0)
            })
            .map_err(|e| format!("统计普通卡数失败: {e}"))?;
        if fallback == 0 {
            return Err("卡池为空，无法抽卡".to_string());
        }
        log::warn!("稀有度 {rarity} 无可用卡，降级为普通卡");
        (1, fallback)
    };

    // OFFSET 随机行而非 ORDER BY RANDOM()：后者要扫全表排序，
    // 且无法用外部种子复现
    let offset = rng.next_u32(count as u32) as i64;
    conn.query_row(
        "SELECT id, name, card_type, rarity, image_path, trivia, source
         FROM cards WHERE rarity = ?1 ORDER BY id LIMIT 1 OFFSET ?2",
        rusqlite::params![target_rarity, offset],
        row_to_card,
    )
    .map_err(|e| format!("读取卡牌失败: {e}"))
}

/// 抽一张卡。券不足返回 Err（契约 §10.4：禁止静默失败）。
pub fn draw(conn: &mut Connection, rng: &mut Rng) -> Result<DrawResult, String> {
    use crate::db::repo::player_stats;

    let tickets = player_stats::get(conn)?.draw_tickets;
    if tickets <= 0 {
        return Err("抽卡券不足".to_string());
    }

    let rarity = pick_rarity(rng.next_u32(100));

    // 扣券与入库必须同事务：中途失败会让用户白扣一张券，
    // 而这是他实打实练出来的
    let tx = conn
        .transaction()
        .map_err(|e| format!("开启抽卡事务失败: {e}"))?;

    let card = pick_card(&tx, rarity, rng)?;
    let now = clock::now();

    tx.execute(
        "INSERT INTO card_collection (card_id, count, first_at, is_new)
         VALUES (?1, 1, ?2, 1)
         ON CONFLICT(card_id) DO UPDATE SET count = count + 1, is_new = 1",
        rusqlite::params![card.id, now],
    )
    .map_err(|e| format!("写入图鉴失败: {e}"))?;

    let (count, first_at): (i64, String) = tx
        .query_row(
            "SELECT count, first_at FROM card_collection WHERE card_id = ?1",
            [card.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| format!("读取收藏记录失败: {e}"))?;

    tx.execute(
        "UPDATE player_stats SET draw_tickets = draw_tickets - 1 WHERE id = 1",
        [],
    )
    .map_err(|e| format!("扣除抽卡券失败: {e}"))?;

    tx.commit().map_err(|e| format!("提交抽卡事务失败: {e}"))?;

    Ok(DrawResult {
        card,
        // first_at 等于本次时间说明是刚插入的行
        is_first: count == 1 && first_at == now,
        count,
        tickets_left: tickets - 1,
    })
}

// ─────────────────────────────────────────────
// Commands
// ─────────────────────────────────────────────

/// contracts §10.4
#[tauri::command]
pub fn draw_card(db: State<Db>) -> Result<DrawResult, String> {
    let mut conn = db
        .0
        .lock()
        .map_err(|e| format!("获取数据库锁失败: {e}"))?;
    let mut rng = Rng::from_clock();
    draw(&mut conn, &mut rng)
}

/// 图鉴：全部卡牌 + 收集状态。未收集的 `count` 为 0，前端显示剪影。
#[tauri::command]
pub fn get_collection(db: State<Db>) -> Result<Vec<CollectionEntry>, String> {
    let conn = db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))?;

    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.name, c.card_type, c.rarity, c.image_path, c.trivia, c.source,
                    COALESCE(k.count, 0) AS count,
                    COALESCE(k.is_new, 0) AS is_new,
                    k.first_at
             FROM cards c
             LEFT JOIN card_collection k ON k.card_id = c.id
             ORDER BY c.card_type, c.rarity DESC, c.id",
        )
        .map_err(|e| format!("准备图鉴查询失败: {e}"))?;

    let rows = stmt
        .query_map([], |r| {
            Ok(CollectionEntry {
                card: row_to_card(r)?,
                count: r.get("count")?,
                is_new: r.get::<_, i64>("is_new")? == 1,
                first_at: r.get("first_at")?,
            })
        })
        .map_err(|e| format!("查询图鉴失败: {e}"))?;

    rows.collect::<Result<_, _>>()
        .map_err(|e| format!("读取图鉴失败: {e}"))
}

/// 清除新卡红点。
#[tauri::command]
pub fn mark_cards_seen(db: State<Db>, card_ids: Vec<i64>) -> Result<(), String> {
    if card_ids.is_empty() {
        return Ok(());
    }
    let conn = db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))?;

    for id in card_ids {
        conn.execute(
            "UPDATE card_collection SET is_new = 0 WHERE card_id = ?1",
            [id],
        )
        .map_err(|e| format!("清除卡牌 {id} 红点失败: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;
    use crate::db::repo::player_stats;
    use crate::test_support::in_memory_db;

    /// 直接用 migration 004 随包分发的真实卡池，不另造测试数据——
    /// 卡池是产品数据的一部分，用假卡测出的行为不能代表线上。
    fn seed(tickets: i64) -> Connection {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();
        player_stats::add_draw_tickets(&conn, tickets).unwrap();
        conn
    }

    fn card_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM cards", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn 券不足时报错而非静默失败() {
        let mut conn = seed(0);
        let err = draw(&mut conn, &mut Rng::new(1)).unwrap_err();
        assert!(err.contains("券"), "错误消息应说明原因: {err}");
    }

    #[test]
    fn 抽卡扣券并写入图鉴() {
        let mut conn = seed(3);
        let result = draw(&mut conn, &mut Rng::new(7)).unwrap();

        assert_eq!(result.tickets_left, 2);
        assert!(result.is_first);
        assert_eq!(result.count, 1);
        assert_eq!(player_stats::get(&conn).unwrap().draw_tickets, 2);
    }

    #[test]
    fn 重复卡累加张数且不再算首次() {
        let mut conn = seed(50);
        // 用固定种子重复抽，总会撞上重复
        let mut rng = Rng::new(3);
        let mut seen_duplicate = false;
        for _ in 0..20 {
            let r = draw(&mut conn, &mut rng).unwrap();
            if !r.is_first {
                assert!(r.count > 1, "重复卡的张数应大于 1");
                seen_duplicate = true;
                break;
            }
        }
        assert!(seen_duplicate, "20 次抽卡未出现重复，卡池或随机逻辑异常");
    }

    #[test]
    fn 抽卡是原子的_失败不扣券() {
        let mut conn = seed(5);
        // 清空卡池模拟异常：券已发出但无卡可给
        conn.execute("DELETE FROM cards", []).unwrap();

        let err = draw(&mut conn, &mut Rng::new(1)).unwrap_err();
        assert!(err.contains("卡池"), "错误消息应指明卡池为空: {err}");
        assert_eq!(
            player_stats::get(&conn).unwrap().draw_tickets,
            5,
            "抽卡失败仍扣了券——用户白练了一场"
        );
    }

    #[test]
    fn 稀有度缺卡时降级而非报错() {
        let mut conn = seed(10);
        // 只留一张普通卡，稀有与传说层为空
        conn.execute("DELETE FROM cards WHERE id != 1", []).unwrap();

        // 券是真金白银练出来的，卡池不全不该让它白费
        for _ in 0..10 {
            let r = draw(&mut conn, &mut Rng::new(99)).unwrap();
            assert_eq!(r.card.id, 1);
        }
    }

    #[test]
    fn 图鉴含未收集卡且计数为零() {
        let mut conn = seed(1);
        draw(&mut conn, &mut Rng::new(5)).unwrap();

        let mut stmt = conn
            .prepare(
                "SELECT c.id, COALESCE(k.count, 0) FROM cards c
                 LEFT JOIN card_collection k ON k.card_id = c.id ORDER BY c.id",
            )
            .unwrap();
        let entries: Vec<(i64, i64)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        let total = card_count(&conn) as usize;
        assert_eq!(entries.len(), total, "图鉴应含全部卡牌，未收集的也要在");
        assert_eq!(
            entries.iter().filter(|(_, c)| *c == 0).count(),
            total - 1,
            "只抽了一张，其余都该是未收集"
        );
    }

    #[test]
    fn 红点可被清除() {
        let mut conn = seed(1);
        let r = draw(&mut conn, &mut Rng::new(11)).unwrap();

        let is_new: i64 = conn
            .query_row(
                "SELECT is_new FROM card_collection WHERE card_id = ?1",
                [r.card.id],
                |x| x.get(0),
            )
            .unwrap();
        assert_eq!(is_new, 1);

        conn.execute(
            "UPDATE card_collection SET is_new = 0 WHERE card_id = ?1",
            [r.card.id],
        )
        .unwrap();
        let after: i64 = conn
            .query_row(
                "SELECT is_new FROM card_collection WHERE card_id = ?1",
                [r.card.id],
                |x| x.get(0),
            )
            .unwrap();
        assert_eq!(after, 0);
    }

    #[test]
    fn 每张卡都记录了来源() {
        // spec F12 验收项：素材来源与许可证必须可追溯
        let conn = seed(1);
        let missing: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM cards WHERE source = '' OR source IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(missing, 0, "存在未记录来源的卡牌");
    }
}
