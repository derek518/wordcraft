//! 版本化数据库迁移。
//!
//! 每个迁移在独立事务中执行：DDL 与 ledger 记录要么同时生效，要么同时回滚。
//! 这一点是 schema-vs-DB drift 的根本防线——ledger 声称已应用而表实际不存在的
//! 情况不可能发生。
//!
//! **已发布的迁移禁止修改**，schema 变更一律新增文件。

use chrono::Utc;
use rusqlite::Connection;

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

/// 新增迁移时在此追加，version 必须严格递增。
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial",
        sql: include_str!("migrations/001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "session_capacity",
        sql: include_str!("migrations/002_session_capacity.sql"),
    },
    Migration {
        version: 3,
        name: "placement",
        sql: include_str!("migrations/003_placement.sql"),
    },
    Migration {
        version: 4,
        name: "card_pool",
        sql: include_str!("migrations/004_card_pool.sql"),
    },
    Migration {
        version: 5,
        name: "painting_pool",
        sql: include_str!("migrations/005_painting_pool.sql"),
    },
    Migration {
        version: 6,
        name: "homestead",
        sql: include_str!("migrations/006_homestead.sql"),
    },
    Migration {
        version: 7,
        name: "season",
        sql: include_str!("migrations/007_season.sql"),
    },
    Migration {
        version: 8,
        name: "boss",
        sql: include_str!("migrations/008_boss.sql"),
    },
    Migration {
        version: 9,
        name: "residents",
        sql: include_str!("migrations/009_residents.sql"),
    },
    Migration {
        version: 10,
        name: "card_pool_v2",
        sql: include_str!("migrations/010_card_pool_v2.sql"),
    },
    Migration {
        version: 11,
        name: "level_cet4",
        sql: include_str!("migrations/011_level_cet4.sql"),
    },
    Migration {
        version: 12,
        name: "single_pace_knob",
        sql: include_str!("migrations/012_single_pace_knob.sql"),
    },
    Migration {
        version: 13,
        name: "frequency_rank",
        sql: include_str!("migrations/013_frequency_rank.sql"),
    },
    Migration {
        version: 14,
        name: "ability",
        sql: include_str!("migrations/014_ability.sql"),
    },
];

/// 执行所有未应用的迁移，返回本次实际应用的版本号。
///
/// 幂等：已应用的迁移会被跳过，重复调用返回空 Vec。
pub fn run(conn: &mut Connection) -> Result<Vec<i64>, String> {
    run_up_to(conn, i64::MAX)
}

/// 只应用到指定版本为止。供测试模拟「老用户的库停在版本 N」。
fn run_up_to(conn: &mut Connection, max_version: i64) -> Result<Vec<i64>, String> {
    ensure_ledger(conn)?;
    let applied = applied_versions(conn)?;

    let mut newly_applied = Vec::new();
    for migration in MIGRATIONS {
        if migration.version > max_version || applied.contains(&migration.version) {
            continue;
        }
        apply(conn, migration)?;
        newly_applied.push(migration.version);
    }
    Ok(newly_applied)
}

/// 当前 schema 版本；空库返回 0。
pub fn current_version(conn: &Connection) -> Result<i64, String> {
    ensure_ledger_readable(conn)?;
    conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )
    .map_err(|e| format!("读取 schema 版本失败: {e}"))
}

fn ensure_ledger(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
           version    INTEGER PRIMARY KEY,
           applied_at TEXT NOT NULL
         );",
    )
    .map_err(|e| format!("创建 schema_migrations 表失败: {e}"))
}

fn ensure_ledger_readable(conn: &Connection) -> Result<(), String> {
    ensure_ledger(conn)
}

fn applied_versions(conn: &Connection) -> Result<Vec<i64>, String> {
    let mut stmt = conn
        .prepare("SELECT version FROM schema_migrations ORDER BY version")
        .map_err(|e| format!("查询已应用迁移失败: {e}"))?;

    let rows = stmt
        .query_map([], |row| row.get::<_, i64>(0))
        .map_err(|e| format!("读取已应用迁移失败: {e}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("解析已应用迁移失败: {e}"))
}

/// 应用一条迁移。
///
/// **外键在事务外关闭。** SQLite 重建一张被引用的表（改 CHECK 只能重建）
/// 必须这么做，官方文档的 "Making Other Kinds Of Table Schema Changes"
/// 就是这个流程：`foreign_keys=OFF` → 事务 → 建新表/拷贝/删旧表/改名 → 提交 → `ON`。
///
/// 两个原因非关不可：
/// - 开着外键时 `DROP TABLE words` 会撞约束，事务直接失败
/// - 更危险的是 `word_states` 声明了 `ON DELETE CASCADE`——DROP 会隐式
///   DELETE 并触发级联，**把用户的全部学习状态一起删掉**，而且不报错
///
/// 关外键不等于放弃校验：提交前跑一次 `foreign_key_check`，留下悬空引用
/// 就整笔回滚。比原先「开着外键因而根本无法重建表」严格。
fn apply(conn: &mut Connection, migration: &Migration) -> Result<(), String> {
    conn.execute_batch("PRAGMA foreign_keys = OFF")
        .map_err(|e| format!("关闭外键失败: {e}"))?;

    let result = apply_in_tx(conn, migration);

    conn.execute_batch("PRAGMA foreign_keys = ON")
        .map_err(|e| format!("恢复外键失败: {e}"))?;
    result
}

fn apply_in_tx(conn: &mut Connection, migration: &Migration) -> Result<(), String> {
    let tx = conn
        .transaction()
        .map_err(|e| format!("开启迁移 {} 事务失败: {e}", migration.version))?;

    tx.execute_batch(migration.sql).map_err(|e| {
        format!(
            "迁移 {} ({}) 执行失败: {e}",
            migration.version, migration.name
        )
    })?;

    // 关了外键就必须自己验。留下悬空引用比迁移失败更难查
    let orphans: i64 = tx
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0))
        .map_err(|e| format!("迁移 {} 外键自检失败: {e}", migration.version))?;
    if orphans > 0 {
        return Err(format!(
            "迁移 {} ({}) 留下 {orphans} 条悬空引用，已回滚",
            migration.version, migration.name
        ));
    }

    tx.execute(
        "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
        (migration.version, Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()),
    )
    .map_err(|e| format!("记录迁移 {} 到 ledger 失败: {e}", migration.version))?;

    tx.commit()
        .map_err(|e| format!("提交迁移 {} 失败: {e}", migration.version))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::in_memory_db;
    use std::collections::HashSet;

    /// 期望的表结构，逐列对照 contracts-v1.md §2。
    ///
    /// 这是 schema-vs-DB drift gate 的核心：ledger 声称迁移已应用，不等于表
    /// 真的按契约建出来了。此处直接读 PRAGMA 与契约比对。
    const EXPECTED_COLUMNS: &[(&str, &[&str])] = &[
        (
            "words",
            &[
                "id", "word", "phonetic", "pos", "meaning", "example_1", "example_2",
                "level", "frequency_band", "zone", "source_edition", "created_at",
                "frequency_rank",
            ],
        ),
        (
            "word_states",
            &[
                "word_id", "difficulty", "stability", "due_at", "fsrs_state",
                "app_state", "reps", "lapses", "question_level", "reinforce_streak",
                "last_review_at", "mastered_at",
            ],
        ),
        (
            "review_logs",
            &[
                "id", "word_id", "session_id", "question_type", "is_correct",
                "reaction_ms", "rating", "difficulty_before", "stability_before",
                "difficulty_after", "stability_after", "reviewed_at",
            ],
        ),
        (
            "sessions",
            &[
                "id", "date", "session_type", "planned_count", "completed_count",
                "is_completed", "xp_earned", "postpone_count", "merged_from",
                "started_at", "finished_at",
            ],
        ),
        (
            "player_stats",
            &[
                "id", "total_xp", "level", "current_streak", "best_streak",
                "last_streak_date", "vocab_estimate", "makeup_cards",
                "pause_used_month", "draw_tickets", "last_grant_month",
                "track_points", "ability_theta", "ability_information",
                "ability_observations",
            ],
        ),
        (
            "daily_records",
            &["date", "is_paused", "eligible_count", "completed_count", "streak_outcome"],
        ),
        (
            "cards",
            &["id", "name", "card_type", "rarity", "image_path", "trivia", "source"],
        ),
        ("card_collection", &["card_id", "count", "first_at", "is_new"]),
        ("settings", &["key", "value"]),
        // migration 003：摸底分级进度，支持分两次完成
        ("placement_results", &["band", "asked", "passed", "is_closed", "consecutive_miss"]),
        ("placement_asked", &["word_id", "asked_at"]),
        // migration 006：家园建造
        ("block_inventory", &["block_type", "owned", "placed"]),
        ("homestead_grid", &["x", "y", "block_type", "placed_at"]),
        // migration 009：家园居民
        ("homestead_residents", &["slot", "card_id", "moved_in_at"]),
        ("block_grants",
         &["id", "source", "source_key", "block_type", "amount", "granted_at"]),
        // migration 007：赛季赛道
        ("season_settlements",
         &["week_start", "sessions_done", "points_earned", "settled_at"]),
    ];

    fn columns_of(conn: &Connection, table: &str) -> Vec<String> {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("PRAGMA 查询失败");
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .expect("读取列名失败");
        rows.map(|r| r.expect("解析列名失败")).collect()
    }

    #[test]
    fn 迁移后每张表的列与契约逐列一致() {
        let mut conn = in_memory_db();
        run(&mut conn).expect("迁移失败");

        for (table, expected) in EXPECTED_COLUMNS {
            let actual = columns_of(&conn, table);
            assert_eq!(
                actual,
                expected.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                "表 `{table}` 的列与 contracts §2 不一致"
            );
        }
    }

    #[test]
    fn 契约中的表全部存在且无多余表() {
        let mut conn = in_memory_db();
        run(&mut conn).expect("迁移失败");

        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master
                 WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            )
            .unwrap();
        let actual: HashSet<String> = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        let mut expected: HashSet<String> = EXPECTED_COLUMNS
            .iter()
            .map(|(t, _)| t.to_string())
            .collect();
        expected.insert("schema_migrations".to_string());
        // sqlite_sequence 不在此列：查询已按 `sqlite_%` 前缀过滤掉内部表

        assert_eq!(actual, expected, "实际表集合与契约不一致");
    }

    /// 带数据的库上重跑全部迁移。
    ///
    /// 迁移测试一向跑在空库上，于是「只有存在外键引用时才失败」的迁移
    /// 能通过每一条测试——011 重建 words 表时正是这样：空库上安然通过，
    /// 用户库上启动即崩。这条测试先造出引用关系，再跑迁移。
    #[test]
    fn 带数据的库能跑完全部迁移() {
        use crate::db::repo::word_states;

        // 停在倒数第二个版本，模拟「老用户的库」，再让最后一条迁移
        // 在有数据的库上跑。先前是删掉最后一条账本再重跑一遍，那要求
        // 迁移 SQL 可重入——`ALTER TABLE ADD COLUMN` 天生做不到
        let last = MIGRATIONS.last().unwrap().version;
        let mut conn = crate::test_support::in_memory_db();
        run_up_to(&mut conn, last - 1).unwrap();

        // 用裸 SQL 而非 words::import：repo 层写的是**当前**版本的列，
        // 而这里的库停在旧版本。老库里的数据本来就是老 schema 写进去的
        conn.execute(
            "INSERT INTO words (word, phonetic, pos, meaning, example_1, example_2,
                                level, frequency_band, zone, source_edition, created_at)
             VALUES ('crystal', '/ˈkrɪstl/', 'n.', '水晶', 'A crystal lights the cave.',
                     '', 'senior', 1, 'newbie', '', ?1)",
            [crate::db::clock::now()],
        )
        .unwrap();

        // 制造外键引用：word_states 与 review_logs 都指向 words
        word_states::upsert(
            &conn,
            &word_states::WordState {
                word_id: 1,
                difficulty: 5.0,
                stability: 3.0,
                due_at: crate::db::clock::now(),
                fsrs_state: 2,
                app_state: "review".into(),
                reps: 3,
                lapses: 0,
                question_level: 2,
                reinforce_streak: 0,
                last_review_at: None,
                mastered_at: None,
            },
        )
        .unwrap();
        conn.execute(
            "INSERT INTO review_logs (word_id, session_id, question_type, is_correct,
             reaction_ms, rating, difficulty_before, stability_before,
             difficulty_after, stability_after, reviewed_at)
             VALUES (1, NULL, 1, 1, 2000, 3, 5.0, 1.0, 4.8, 3.0, ?1)",
            [crate::db::clock::now()],
        )
        .unwrap();

        let applied = run(&mut conn).expect("最后一条迁移在有数据的库上失败");
        assert_eq!(applied, vec![last], "应恰好补上最后一条迁移");

        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM words", [], |r| r.get::<_, i64>(0)).unwrap(),
            1,
            "重建表不该丢词"
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM word_states", [], |r| r.get::<_, i64>(0)).unwrap(),
            1,
            "学习状态必须原样保留"
        );
        let orphans: i64 = conn
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0))
            .unwrap_or(0);
        assert_eq!(orphans, 0, "重建后不该留下悬空引用");
    }

    #[test]
    fn 重复执行幂等() {
        let mut conn = in_memory_db();

        let first = run(&mut conn).expect("首次迁移失败");
        assert_eq!(
            first,
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14],
            "首次应用全部迁移"
        );

        let second = run(&mut conn).expect("重复迁移失败");
        assert!(second.is_empty(), "重复执行不应再应用任何迁移");

        let third = run(&mut conn).expect("第三次迁移失败");
        assert!(third.is_empty());

        // 数据未被重置：001 插 9 个键，002 追加 session_word_count，012 又删掉它
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM settings", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 9, "settings 默认值被重复插入或丢失");
    }

    #[test]
    fn 版本号正确推进() {
        let mut conn = in_memory_db();
        assert_eq!(current_version(&conn).unwrap(), 0, "空库版本应为 0");

        run(&mut conn).unwrap();
        let expected = MIGRATIONS.last().unwrap().version;
        assert_eq!(current_version(&conn).unwrap(), expected);
    }

    #[test]
    fn 单行表约束生效() {
        let mut conn = in_memory_db();
        run(&mut conn).unwrap();

        // player_stats 只允许 id=1 一行
        let dup = conn.execute("INSERT INTO player_stats (id) VALUES (2)", []);
        assert!(dup.is_err(), "player_stats 的 id=1 约束未生效");

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM player_stats", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn 受控词表约束拒绝非法值() {
        let mut conn = in_memory_db();
        run(&mut conn).unwrap();

        let insert = |zone: &str, band: i64, level: &str| {
            conn.execute(
                "INSERT INTO words
                 (word, pos, meaning, example_1, level, frequency_band, zone, created_at)
                 VALUES (?1, 'n.', '测试', 'A test sentence.', ?2, ?3, ?4, '2026-01-01T00:00:00Z')",
                (format!("w{zone}{band}{level}"), level, band, zone),
            )
        };

        assert!(insert("newbie", 1, "junior").is_ok(), "合法值应被接受");
        assert!(insert("nowhere", 1, "junior").is_err(), "非法 zone 未被拒绝");
        assert!(insert("newbie", 9, "junior").is_err(), "越界 frequency_band 未被拒绝");
        assert!(insert("newbie", 1, "college").is_err(), "非法 level 未被拒绝");
    }

    #[test]
    fn 状态机受控值约束生效() {
        let mut conn = in_memory_db();
        run(&mut conn).unwrap();
        conn.execute(
            "INSERT INTO words (id, word, pos, meaning, example_1, level, frequency_band, zone, created_at)
             VALUES (1, 'test', 'n.', '测试', 'A test.', 'junior', 1, 'newbie', '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();

        let insert_state = |app_state: &str| {
            conn.execute(
                "INSERT OR REPLACE INTO word_states (word_id, due_at, app_state)
                 VALUES (1, '2026-01-01T00:00:00Z', ?1)",
                [app_state],
            )
        };

        for valid in ["new", "learning", "reinforcing", "review", "mastered"] {
            assert!(insert_state(valid).is_ok(), "合法 app_state `{valid}` 被拒绝");
        }
        assert!(insert_state("mastered_maybe").is_err(), "非法 app_state 未被拒绝");
    }

    #[test]
    fn 延后次数上限被数据库约束兜住() {
        let mut conn = in_memory_db();
        run(&mut conn).unwrap();

        // spec F1：每时段最多延后 3 次。业务层会先拦截，但数据库是最后一道防线。
        let insert = |n: i64| {
            conn.execute(
                "INSERT OR REPLACE INTO sessions (id, date, session_type, planned_count, postpone_count)
                 VALUES (1, '2026-08-05', 'morning', 5, ?1)",
                [n],
            )
        };
        assert!(insert(3).is_ok());
        assert!(insert(4).is_err(), "postpone_count 上限约束未生效");
    }
}
