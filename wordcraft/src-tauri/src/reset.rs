//! 清空学习数据，把应用恢复成「全新一台」。
//!
//! ## 为什么必须有这个
//!
//! 家长装好应用后总会先自己点几下试试。那些作答会进 FSRS：成年人的正确率
//! 与反应时间会把上百个词标成 `review`（平均间隔数十天）甚至 `mastered`，
//! 孩子接手后这些词一个月内不会出现，系统认定「已掌握」——而这个判断
//! 来自另一个人。能力评估越准，这种污染越致命，因为它会被当成真信号。
//!
//! ## 边界
//!
//! 清：全部学习记录**与游戏进度**。等级、方块、卡牌收藏一并清空，不是
//! 为了彻底，而是因为它们互相引用——只清作答记录会留下「魔王已讨伐但
//! 那个词又变回生词」这类自相矛盾的状态。而且早期的升级与解锁本身就是
//! 动机设计的一部分，让孩子从 Lv.11 起步等于把这段体验删掉。
//!
//! 留：词库、卡牌目录、家长配置（时段 / 每日新词 / 学习范围 / 发音 / 自启）。

use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

use crate::db::repo::settings;
use crate::db::Db;

/// 清空的表及其行数。命令不能返回 `Ok(())` 了事——
/// 「点了没反应」和「清干净了」在界面上必须能区分。
#[derive(Debug, Default, PartialEq, Serialize)]
pub struct ResetSummary {
    pub cleared: Vec<(String, i64)>,
    pub total_rows: i64,
}

/// 按外键依赖顺序清空：子表在前，父表在后。
///
/// `words` 与 `cards` 不在其中——它们是词库与目录，不是进度。
const TABLES: &[&str] = &[
    "review_logs",         // → sessions, words
    "sessions",            //
    "homestead_grid",      // → block_inventory
    "homestead_residents", // → cards
    "card_collection",     // → cards
    "block_grants",        //
    "word_states",         // → words
    "placement_asked",     // → words
    "placement_results",   //
    "daily_records",       //
    "season_settlements",  //
];

/// 需要回到初值的设置键。其余是家长配置，保留。
const SETTINGS_RESET: &[(&str, &str)] = &[
    ("onboarding_done", "false"),
    ("placement_stage", "0"),
    ("daily_pause_date", ""),
    ("season_milestone_seen", "0"),
    (settings::POSTPONE_UNTIL, ""),
    (settings::POSTPONE_TYPE, ""),
];

pub fn reset_learning_data(conn: &mut Connection) -> Result<ResetSummary, String> {
    let tx = conn
        .transaction()
        .map_err(|e| format!("开启重置事务失败: {e}"))?;

    let mut summary = ResetSummary::default();
    for table in TABLES {
        let n: i64 = tx
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .map_err(|e| format!("统计 {table} 失败: {e}"))?;
        tx.execute(&format!("DELETE FROM {table}"), [])
            .map_err(|e| format!("清空 {table} 失败: {e}"))?;
        if n > 0 {
            summary.cleared.push(((*table).to_string(), n));
            summary.total_rows += n;
        }
    }

    // block_inventory 的三行是**结构种子**（三种方块类型），不是进度；
    // 进度是它的计数。删掉会让 homestead_grid 的外键无处可指
    let blocks: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM block_inventory WHERE owned > 0 OR placed > 0",
            [],
            |r| r.get(0),
        )
        .map_err(|e| format!("统计方块库存失败: {e}"))?;
    tx.execute("UPDATE block_inventory SET owned = 0, placed = 0", [])
        .map_err(|e| format!("清空方块库存失败: {e}"))?;
    if blocks > 0 {
        summary.cleared.push(("block_inventory".into(), blocks));
        summary.total_rows += blocks;
    }

    // 玩家统计是单行，删掉再按 schema 默认值重建——
    // 逐列写 0 会在加新列时悄悄漏掉那一列
    tx.execute("DELETE FROM player_stats", [])
        .map_err(|e| format!("清空 player_stats 失败: {e}"))?;
    tx.execute("INSERT INTO player_stats (id) VALUES (1)", [])
        .map_err(|e| format!("重建 player_stats 失败: {e}"))?;

    for (key, value) in SETTINGS_RESET {
        tx.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            rusqlite::params![key, value],
        )
        .map_err(|e| format!("重置设置 {key} 失败: {e}"))?;
    }

    // 删错顺序会留下悬空引用，而 SQLite 默认不会因此报错——
    // 提交前自查，别把一个坏掉的库交给用户
    let dangling: i64 = tx
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .map_err(|e| format!("外键自查失败: {e}"))?;
    if dangling > 0 {
        return Err(format!("重置后存在 {dangling} 条悬空引用，已回滚"));
    }

    tx.commit().map_err(|e| format!("提交重置失败: {e}"))?;
    log::info!(
        "学习数据已重置，共清空 {} 行：{:?}",
        summary.total_rows,
        summary.cleared
    );
    Ok(summary)
}

/// contracts §3.6：清空学习数据。返回清空明细而非 `Ok(())`——
/// 这个操作不可逆，用户有权知道到底动了什么。
#[tauri::command]
pub fn reset_learning_data_cmd(db: State<Db>) -> Result<ResetSummary, String> {
    let mut conn = db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))?;
    reset_learning_data(&mut conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::{player_stats, review_logs, sessions, word_states, words};
    use crate::db::{clock, migrations};
    use crate::test_support::in_memory_db;

    /// 造一个「家长试用过」的库：词库 + 作答记录 + 等级 + 方块。
    fn used_db() -> Connection {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();

        let items: Vec<words::WordImport> = (0..3)
            .map(|i| {
                let w = format!("rw{}", (b'a' + i as u8) as char);
                words::WordImport {
                    example_1: format!("A {w} appears."),
                    word: w,
                    phonetic: "/w/".into(),
                    pos: "n.".into(),
                    meaning: format!("释义{i}"),
                    example_2: String::new(),
                    level: "senior".into(),
                    frequency_band: 1,
                    frequency_rank: None,
                    zone: "newbie".into(),
                    source_edition: String::new(),
                }
            })
            .collect();
        words::import(&mut conn, &items).unwrap();

        let today = clock::today();
        let session = sessions::start(&conn, &today, "morning", 10, &clock::now()).unwrap();

        for id in 1..=3 {
            word_states::upsert(
                &conn,
                &word_states::WordState {
                    word_id: id,
                    difficulty: 5.0,
                    stability: 30.0,
                    due_at: clock::due_in_days(30.0),
                    fsrs_state: 2,
                    app_state: "review".into(),
                    reps: 4,
                    lapses: 0,
                    question_level: 2,
                    reinforce_streak: 0,
                    last_review_at: Some(clock::now()),
                    mastered_at: None,
                },
            )
            .unwrap();
            review_logs::insert(
                &conn,
                &review_logs::NewReviewLog {
                    word_id: id,
                    session_id: Some(session.id),
                    question_type: 1,
                    is_correct: true,
                    reaction_ms: 800,
                    rating: 4,
                    difficulty_before: 5.0,
                    stability_before: 1.0,
                    difficulty_after: 5.0,
                    stability_after: 30.0,
                },
                &clock::now(),
            )
            .unwrap();
        }

        // 家园：方块库存与已摆放的格子
        conn.execute(
            "UPDATE block_inventory SET owned = 40, placed = 2 WHERE block_type = 'normal'",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO homestead_grid (x, y, block_type, placed_at) VALUES (1,1,'normal',?1),(2,2,'normal',?1)",
            [clock::now()],
        )
        .unwrap();

        player_stats::add_xp(&conn, 5000).unwrap();
        player_stats::add_draw_tickets(&conn, 7).unwrap();
        settings::set(&conn, "onboarding_done", "true").unwrap();
        settings::set(&conn, "placement_stage", "2").unwrap();
        settings::set(&conn, "daily_new_words", "42").unwrap();
        settings::set(&conn, "study_level", "cet4").unwrap();
        conn
    }

    fn count(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn 清空作答与状态但保留词库() {
        let mut conn = used_db();
        assert_eq!(count(&conn, "words"), 3);
        assert_eq!(count(&conn, "review_logs"), 3);

        let s = reset_learning_data(&mut conn).unwrap();

        assert_eq!(count(&conn, "review_logs"), 0);
        assert_eq!(count(&conn, "word_states"), 0);
        assert_eq!(count(&conn, "sessions"), 0);
        // 词库不是进度。清掉它等于让用户重新导入五千个词
        assert_eq!(count(&conn, "words"), 3, "词库不该被清");
        assert_eq!(count(&conn, "cards"), 42, "卡牌目录不该被清");
        assert!(s.total_rows > 0, "摘要必须反映实际清了东西");
    }

    #[test]
    fn 等级与抽卡券归零() {
        let mut conn = used_db();
        let before = player_stats::get(&conn).unwrap();
        assert!(before.total_xp >= 5000 && before.draw_tickets >= 7);

        reset_learning_data(&mut conn).unwrap();

        let after = player_stats::get(&conn).unwrap();
        // 从 Lv.11 起步等于把早期的升级与解锁体验删掉,
        // 而那正是这个应用留住 ADHD 用户的主要手段
        assert_eq!(after.total_xp, 0);
        assert_eq!(after.draw_tickets, 0);
        assert_eq!(after.current_streak, 0);
        assert_eq!(count(&conn, "player_stats"), 1, "单行必须重建而非消失");
    }

    #[test]
    fn 摸底状态回到未开始但家长配置保留() {
        let mut conn = used_db();
        reset_learning_data(&mut conn).unwrap();

        let get = |k: &str| settings::get(&conn, k).unwrap().unwrap_or_default();
        assert_eq!(get("onboarding_done"), "false", "孩子应重新走一遍引导");
        assert_eq!(get("placement_stage"), "0");
        // 时段、每日新词、学习范围是家长配好的,重置不该让人再配一遍
        assert_eq!(get("daily_new_words"), "42");
        assert_eq!(get("study_level"), "cet4");
    }

    #[test]
    fn 方块计数归零但类型行保留() {
        let mut conn = used_db();
        assert_eq!(count(&conn, "homestead_grid"), 2);

        reset_learning_data(&mut conn).unwrap();

        assert_eq!(count(&conn, "homestead_grid"), 0, "已摆放的格子是进度");
        let (owned, placed): (i64, i64) = conn
            .query_row(
                "SELECT owned, placed FROM block_inventory WHERE block_type = 'normal'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((owned, placed), (0, 0), "库存计数是进度，应归零");
        // 三种方块类型是 schema 种子,删掉会让 homestead_grid 的外键无处可指
        assert_eq!(count(&conn, "block_inventory"), 3, "类型行不该被删");
    }

    #[test]
    fn 重置后不留悬空引用() {
        let mut conn = used_db();
        reset_learning_data(&mut conn).unwrap();
        assert_eq!(
            count(&conn, "pragma_foreign_key_check"),
            0,
            "删除顺序错会留下悬空引用,而 SQLite 默认不报错"
        );
    }

    #[test]
    fn 重复重置不报错且摘要为空() {
        let mut conn = used_db();
        reset_learning_data(&mut conn).unwrap();
        let second = reset_learning_data(&mut conn).unwrap();
        // 已经干净的库再重置一次应当是空操作,而不是失败
        assert_eq!(second.total_rows, 0);
        assert!(second.cleared.is_empty());
    }

    #[test]
    fn 全新库上重置不报错() {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();
        let s = reset_learning_data(&mut conn).unwrap();
        assert_eq!(s.total_rows, 0);
        assert_eq!(count(&conn, "player_stats"), 1);
    }
}
