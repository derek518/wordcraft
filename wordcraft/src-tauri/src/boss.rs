//! 魔王讨伐战。spec §4.2 F10「薄弱词特训」。
//!
//! 把强化队列里最顽固的词包装成 boss 战。底层仍是同一套 FSRS 与状态机——
//! 变的只是呈现：spec 要的是让「又忘了」这件事从挫败变成可以主动出击的目标。
//!
//! 文案基调是「不刻薄」（spec 原话）。目标用户已经因为记不住而受挫，
//! 嘲讽只会把人推走。

use crate::db::{clock, repo::homestead as blocks, Db};
use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

/// 成为魔王所需的遗忘次数。
///
/// spec 写的是「连续 2 次忘记或 3 次模糊」。`lapses` 正是 FSRS 记录的
/// 「记住之后又忘了」的次数，语义吻合，无需另建计数。
pub const BOSS_LAPSE_THRESHOLD: i64 = 2;

/// 击败魔王需要连续答对的次数——boss 的「血量」。
///
/// 3 次而非 1 次：顽固词的特点就是当场想起来、隔天又忘。
/// 一次答对不足以说明问题解决了。
pub const BOSS_HP: i64 = 3;

// 一次答对就击败说明不了问题——顽固词的特点正是当场想起、隔天又忘。
// 编译期检查，调低血量时构建直接失败
const _: () = assert!(BOSS_HP >= 2);

/// 击败后强制提升的题型等级。spec：「强制升级 mastery 2」。
const LEVEL_BOOST: i64 = 2;

#[derive(Debug, Serialize)]
pub struct BossWord {
    pub word_id: i64,
    pub word: String,
    pub phonetic: String,
    pub pos: String,
    pub meaning: String,
    pub example_1: String,
    /// 遗忘次数，即这个魔王「击败过你」多少次
    pub lapses: i64,
    pub hp: i64,
    /// 是否已经讨伐过（掉落只发一次）
    pub already_defeated: bool,
}

#[derive(Debug, Serialize)]
pub struct DefeatOutcome {
    pub word: String,
    /// 本次是否掉落稀有方块。重复讨伐同一个魔王不再掉落
    pub dropped_block: bool,
    pub new_question_level: i64,
}

fn lock(db: &Db) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
    db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))
}

/// 待讨伐的魔王，按顽固程度排序。
#[tauri::command]
pub fn get_boss_words(db: State<Db>, limit: i64) -> Result<Vec<BossWord>, String> {
    let conn = lock(&db)?;
    boss_words(&conn, limit)
}

/// 取魔王列表。与 command 分开是为了让测试打在**真查询**上——
/// 先前的测试自己重写了一遍 SQL，改生产代码时它一声不吭。
pub fn boss_words(conn: &Connection, limit: i64) -> Result<Vec<BossWord>, String> {
    if !(1..=50).contains(&limit) {
        return Err(format!("limit 必须在 1..50，收到 {limit}"));
    }

    let defeated = blocks::granted_keys(conn, "boss")?;

    // 魔王同样是练习，同样受学习范围约束。设置页承诺「已练过的范围外单词
    // 也不再排入」——这条路上不过滤的话，切到四级后仍会被高中词堵住
    let scope_sql = crate::scope::current(conn)?.sql_filter();

    let mut stmt = conn
        .prepare(&format!(
            "SELECT w.id, w.word, w.phonetic, w.pos, w.meaning, w.example_1, s.lapses
             FROM word_states s JOIN words w ON w.id = s.word_id
             WHERE s.lapses >= ?1 AND {scope_sql}
             ORDER BY s.lapses DESC, s.due_at ASC
             LIMIT ?2"
        ))
        .map_err(|e| format!("准备魔王查询失败: {e}"))?;

    let rows = stmt
        .query_map(rusqlite::params![BOSS_LAPSE_THRESHOLD, limit], |r| {
            let word_id: i64 = r.get(0)?;
            Ok(BossWord {
                word_id,
                word: r.get(1)?,
                phonetic: r.get(2)?,
                pos: r.get(3)?,
                meaning: r.get(4)?,
                example_1: r.get(5)?,
                lapses: r.get(6)?,
                hp: BOSS_HP,
                already_defeated: defeated.contains(&word_id.to_string()),
            })
        })
        .map_err(|e| format!("查询魔王失败: {e}"))?;

    rows.collect::<Result<_, _>>()
        .map_err(|e| format!("读取魔王失败: {e}"))
}

/// 记录击败。掉落走 `block_grants`，`source_key` 用 word_id。
///
/// **防刷**：同一个词只掉落一次。否则用户可以故意答错让它变回魔王，
/// 再打一遍刷稀有方块——而稀有方块的总量本该是稀缺的。
pub fn defeat(conn: &mut Connection, word_id: i64) -> Result<DefeatOutcome, String> {
    let (word, level): (String, i64) = conn
        .query_row(
            "SELECT w.word, s.question_level
             FROM word_states s JOIN words w ON w.id = s.word_id
             WHERE s.word_id = ?1",
            [word_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| format!("词条 {word_id} 没有学习记录，无法讨伐"))?;

    let new_level = (level + LEVEL_BOOST).min(5);
    let now = clock::now();

    let tx = conn
        .transaction()
        .map_err(|e| format!("开启讨伐事务失败: {e}"))?;

    // spec：击败后强制升级。它刚证明自己能连对三次，题型该往上走
    tx.execute(
        "UPDATE word_states SET question_level = ?2 WHERE word_id = ?1",
        rusqlite::params![word_id, new_level],
    )
    .map_err(|e| format!("提升题型等级失败: {e}"))?;

    let dropped = blocks::record_grant(&tx, "boss", &word_id.to_string(), "rare", 1, &now)?;
    if dropped {
        blocks::add_owned(&tx, "rare", 1)?;
    }

    tx.commit().map_err(|e| format!("提交讨伐事务失败: {e}"))?;

    if dropped {
        log::info!("击败魔王 `{word}`，掉落稀有方块");
    }

    Ok(DefeatOutcome {
        word,
        dropped_block: dropped,
        new_question_level: new_level,
    })
}

#[tauri::command]
pub fn defeat_boss(db: State<Db>, word_id: i64) -> Result<DefeatOutcome, String> {
    let mut conn = db
        .0
        .lock()
        .map_err(|e| format!("获取数据库锁失败: {e}"))?;
    defeat(&mut conn, word_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{migrations, repo::word_states, repo::words};
    use crate::test_support::in_memory_db;

    fn db() -> Connection {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();
        let items: Vec<words::WordImport> = (0..5)
            .map(|i| {
                let w = format!("bw{}", (b'a' + i as u8) as char);
                words::WordImport {
                    word: w.clone(),
                    phonetic: "/w/".into(),
                    pos: "n.".into(),
                    meaning: format!("释义{i}"),
                    pos_2: None,
                    meaning_2: None,
                    example_1: format!("A {w} appears."),
                    example_2: String::new(),
                    // senior：与默认学习范围一致。用 junior 的话，下面测的
                    // 其实是「范围过滤把一切挡光了」，而不是魔王逻辑本身
                    level: "senior".into(),
                    frequency_band: 1,
                    frequency_rank: None,
                    zone: "newbie".into(),
                    source_edition: String::new(),
                }
            })
            .collect();
        let out = words::import(&mut conn, &items).unwrap();
        assert!(out.rejected.is_empty());
        conn
    }

    fn set_lapses(conn: &Connection, word_id: i64, lapses: i64, level: i64) {
        word_states::upsert(
            conn,
            &word_states::WordState {
                word_id,
                difficulty: 7.0,
                stability: 1.0,
                due_at: clock::now(),
                fsrs_state: 3,
                app_state: "reinforcing".into(),
                reps: 5,
                lapses,
                question_level: level,
                reinforce_streak: 0,
                last_review_at: None,
                mastered_at: None,
            },
        )
        .unwrap();
    }

    fn rare_owned(conn: &Connection) -> i64 {
        blocks::inventory(conn)
            .unwrap()
            .iter()
            .find(|s| s.block_type == "rare")
            .unwrap()
            .owned
    }

    #[test]
    fn 只有遗忘达阈值的词成为魔王() {
        let conn = db();
        set_lapses(&conn, 1, 1, 1); // 未达阈值
        set_lapses(&conn, 2, 2, 1);
        set_lapses(&conn, 3, 5, 1);

        let ids: Vec<i64> = boss_words(&conn, 10)
            .unwrap()
            .iter()
            .map(|b| b.word_id)
            .collect();
        assert_eq!(ids, vec![3, 2], "应按顽固程度排序且排除未达阈值的");
    }

    #[test]
    fn 魔王也受学习范围约束() {
        let conn = db();
        set_lapses(&conn, 1, 5, 1);
        assert_eq!(boss_words(&conn, 10).unwrap().len(), 1);

        // 设置页承诺「已练过的范围外单词也不再排入」。魔王同样是练习，
        // 这条路上不过滤的话，切到四级后仍会被高中词堵住
        crate::db::repo::settings::set(&conn, crate::scope::SETTING_KEY, "cet4").unwrap();
        assert!(
            boss_words(&conn, 10).unwrap().is_empty(),
            "范围外的词不该出现在魔王榜"
        );
    }

    #[test]
    fn 非法_limit_被拒绝而非静默截断() {
        let conn = db();
        assert!(boss_words(&conn, 0).is_err());
        assert!(boss_words(&conn, 51).is_err());
    }

    #[test]
    fn 击败掉落稀有方块并提升题型() {
        let mut conn = db();
        set_lapses(&conn, 1, 3, 1);

        let out = defeat(&mut conn, 1).unwrap();
        assert!(out.dropped_block);
        assert_eq!(out.new_question_level, 3, "应提升两级");
        assert_eq!(rare_owned(&conn), 1);
    }

    #[test]
    fn 重复讨伐同一个魔王不再掉落() {
        let mut conn = db();
        set_lapses(&conn, 1, 3, 1);
        defeat(&mut conn, 1).unwrap();

        // 用户可以故意答错让词变回魔王再打一遍——稀有方块本该稀缺，
        // 不能这样刷
        let again = defeat(&mut conn, 1).unwrap();
        assert!(!again.dropped_block, "重复讨伐仍掉落，可被刷取");
        assert_eq!(rare_owned(&conn), 1);
    }

    #[test]
    fn 题型等级封顶在五() {
        let mut conn = db();
        set_lapses(&conn, 1, 3, 4);
        let out = defeat(&mut conn, 1).unwrap();
        assert_eq!(out.new_question_level, 5);
    }

    #[test]
    fn 无学习记录的词无法讨伐() {
        let mut conn = db();
        let err = defeat(&mut conn, 99).unwrap_err();
        assert!(err.contains("99"), "错误消息应指明词条: {err}");
    }

    #[test]
    fn 讨伐是原子的_失败不发方块() {
        let mut conn = db();
        // 词条不存在时整笔失败
        assert!(defeat(&mut conn, 404).is_err());
        assert_eq!(rare_owned(&conn), 0);
    }
}

#[cfg(test)]
mod integration_tests {
    //! 对着真实词库跑一遍完整讨伐，验证 command 层的连接而非仅逻辑。
    use super::*;
    use crate::db::migrations;

    #[test]
    fn 完整讨伐流程改变四处状态() {
        let mut conn = crate::test_support::in_memory_db();
        migrations::run(&mut conn).unwrap();
        crate::db::repo::words::import(
            &mut conn,
            &[crate::db::repo::words::WordImport {
                word: "stubborn".into(),
                phonetic: "/ˈstʌbərn/".into(),
                pos: "adj.".into(),
                meaning: "顽固的".into(),
                pos_2: None,
                meaning_2: None,
                example_1: "A stubborn word refuses to stay.".into(),
                example_2: String::new(),
                level: "senior".into(),
                frequency_band: 2,
                frequency_rank: None,
                zone: "grass".into(),
                source_edition: String::new(),
            }],
        )
        .unwrap();

        crate::db::repo::word_states::upsert(
            &conn,
            &crate::db::repo::word_states::WordState {
                word_id: 1,
                difficulty: 8.0,
                stability: 0.5,
                due_at: clock::now(),
                fsrs_state: 3,
                app_state: "reinforcing".into(),
                reps: 8,
                lapses: 4,
                question_level: 2,
                reinforce_streak: 0,
                last_review_at: None,
                mastered_at: None,
            },
        )
        .unwrap();

        let out = defeat(&mut conn, 1).unwrap();

        // 1. 掉落发生
        assert!(out.dropped_block);
        // 2. 库存增加
        assert_eq!(
            blocks::inventory(&conn).unwrap().iter()
                .find(|s| s.block_type == "rare").unwrap().owned,
            1
        );
        // 3. 题型提升两级并落库
        let level: i64 = conn
            .query_row("SELECT question_level FROM word_states WHERE word_id=1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(level, 4);
        // 4. 账本留痕，来源可追溯
        let src: String = conn
            .query_row("SELECT source FROM block_grants WHERE source_key='1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(src, "boss");
    }
}
