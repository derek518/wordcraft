//! FSRS 状态与产品状态机的持久化。
//!
//! ADR-2：本模块**不做任何 FSRS 计算**——difficulty/stability/due_at 由前端
//! ts-fsrs 算出后下发，这里只负责存取与校验。
//! ADR-6：`fsrs_state`（算法所有）与 `app_state`（产品状态机）语义不同，分列存储。

use rusqlite::{Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

pub const APP_STATES: [&str; 5] = ["new", "learning", "reinforcing", "review", "mastered"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordState {
    pub word_id: i64,
    pub difficulty: f64,
    pub stability: f64,
    pub due_at: String,
    pub fsrs_state: i64,
    pub app_state: String,
    pub reps: i64,
    pub lapses: i64,
    pub question_level: i64,
    pub reinforce_streak: i64,
    pub last_review_at: Option<String>,
    pub mastered_at: Option<String>,
}

fn row_to_state(row: &Row) -> rusqlite::Result<WordState> {
    Ok(WordState {
        word_id: row.get("word_id")?,
        difficulty: row.get("difficulty")?,
        stability: row.get("stability")?,
        due_at: row.get("due_at")?,
        fsrs_state: row.get("fsrs_state")?,
        app_state: row.get("app_state")?,
        reps: row.get("reps")?,
        lapses: row.get("lapses")?,
        question_level: row.get("question_level")?,
        reinforce_streak: row.get("reinforce_streak")?,
        last_review_at: row.get("last_review_at")?,
        mastered_at: row.get("mastered_at")?,
    })
}

pub fn get(conn: &Connection, word_id: i64) -> Result<Option<WordState>, String> {
    conn.query_row(
        "SELECT * FROM word_states WHERE word_id = ?1",
        [word_id],
        row_to_state,
    )
    .optional()
    .map_err(|e| format!("查询词状态 {word_id} 失败: {e}"))
}

/// 写入或更新词状态。
///
/// 校验受控值——数据库的 CHECK 是最后防线，但错误应在这里就被拦下并给出
/// 可诊断的消息，而不是让上层拿到一句 SQLite 约束错误。
pub fn upsert(conn: &Connection, s: &WordState) -> Result<(), String> {
    if !APP_STATES.contains(&s.app_state.as_str()) {
        return Err(format!("非法 app_state `{}`", s.app_state));
    }
    if !(0..=3).contains(&s.fsrs_state) {
        return Err(format!("非法 fsrs_state {}", s.fsrs_state));
    }
    if !(1..=5).contains(&s.question_level) {
        return Err(format!("非法 question_level {}", s.question_level));
    }
    if s.stability < 0.0 || s.difficulty < 0.0 {
        return Err("stability / difficulty 不能为负".into());
    }

    conn.execute(
        "INSERT INTO word_states
           (word_id, difficulty, stability, due_at, fsrs_state, app_state,
            reps, lapses, question_level, reinforce_streak, last_review_at, mastered_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(word_id) DO UPDATE SET
           difficulty = excluded.difficulty,
           stability = excluded.stability,
           due_at = excluded.due_at,
           fsrs_state = excluded.fsrs_state,
           app_state = excluded.app_state,
           reps = excluded.reps,
           lapses = excluded.lapses,
           question_level = excluded.question_level,
           reinforce_streak = excluded.reinforce_streak,
           last_review_at = excluded.last_review_at,
           mastered_at = excluded.mastered_at",
        rusqlite::params![
            s.word_id,
            s.difficulty,
            s.stability,
            s.due_at,
            s.fsrs_state,
            s.app_state,
            s.reps,
            s.lapses,
            s.question_level,
            s.reinforce_streak,
            s.last_review_at,
            s.mastered_at,
        ],
    )
    .map_err(|e| format!("写入词状态 {} 失败: {e}", s.word_id))?;
    Ok(())
}

/// 处于指定业务状态的词数。强化池大小（自适应控制的输入）由此得出。
pub fn count_by_app_state(conn: &Connection, app_state: &str) -> Result<i64, String> {
    conn.query_row(
        "SELECT COUNT(*) FROM word_states WHERE app_state = ?1",
        [app_state],
        |r| r.get(0),
    )
    .map_err(|e| format!("统计 `{app_state}` 状态词数失败: {e}"))
}

/// 各业务状态的词数分布。用于仪表盘的五段色条。
pub fn distribution(conn: &Connection) -> Result<Vec<(String, i64)>, String> {
    let mut stmt = conn
        .prepare("SELECT app_state, COUNT(*) FROM word_states GROUP BY app_state")
        .map_err(|e| format!("准备分布查询失败: {e}"))?;

    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        .map_err(|e| format!("查询状态分布失败: {e}"))?;

    rows.collect::<Result<_, _>>()
        .map_err(|e| format!("读取状态分布失败: {e}"))
}

/// 尚未真正学过的词数（限定在学习范围内）。
///
/// 「学过」的判据是 **`reps > 0`——真的作答过**，不是「有没有状态行」。
/// 摸底会为一千多个词预建状态行，那是「估计你可能认识」，不是「你练过」。
///
/// 按行数算的旧口径让界面显示「已点亮 1589/3657」，而用户实际只练过 151 个词——
/// 十倍的虚高，且正是它让人以为「都这个进度了怎么还在背 the」。
/// 家园方块的发放早已按 `reps > 0` 判定，此处对齐同一口径。
pub fn untouched_count(conn: &Connection, scope_sql: &str) -> Result<i64, String> {
    conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM words w
             WHERE {scope_sql}
               AND NOT EXISTS (
                 SELECT 1 FROM word_states s WHERE s.word_id = w.id AND s.reps > 0
               )"
        ),
        [],
        |r| r.get(0),
    )
    .map_err(|e| format!("统计未学习词数失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repo::words::{self, WordImport};
    use crate::db::{clock, migrations};
    use crate::test_support::in_memory_db;

    fn db_with_words(n: usize) -> Connection {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();

        let items: Vec<WordImport> = (0..n)
            .map(|i| {
                // 单词必须是纯小写字母：validate 会拒绝含数字的词
                let w = format!("word{}", (b'a' + i as u8) as char);
                WordImport {
                    pos_2: None,
                    meaning_2: None,
                    example_1: format!("A {w} appears here."),
                    word: w,
                    phonetic: "/w/".into(),
                    pos: "n.".into(),
                    meaning: format!("释义{i}"),
                    example_2: String::new(),
                    level: "junior".into(),
                    frequency_band: 1,
                    frequency_rank: None,
                    zone: "newbie".into(),
                    source_edition: String::new(),
                }
            })
            .collect();

        let outcome = words::import(&mut conn, &items).unwrap();
        // 夹具必须自证前置条件成立，否则导入失败会以外键错误的形式伪装出现
        assert!(
            outcome.rejected.is_empty(),
            "测试夹具的词条未通过校验: {:?}",
            outcome.rejected
        );
        assert_eq!(outcome.inserted, n as i64);
        conn
    }

    fn state(word_id: i64, app_state: &str) -> WordState {
        WordState {
            word_id,
            difficulty: 5.0,
            stability: 3.0,
            due_at: clock::now(),
            fsrs_state: 1,
            app_state: app_state.into(),
            reps: 1,
            lapses: 0,
            question_level: 1,
            reinforce_streak: 0,
            last_review_at: Some(clock::now()),
            mastered_at: None,
        }
    }

    #[test]
    fn 写入后可读回全部字段() {
        let conn = db_with_words(1);
        let s = state(1, "learning");
        upsert(&conn, &s).unwrap();

        let got = get(&conn, 1).unwrap().unwrap();
        assert_eq!(got.app_state, "learning");
        assert_eq!(got.difficulty, 5.0);
        assert_eq!(got.stability, 3.0);
        assert_eq!(got.fsrs_state, 1);
        assert_eq!(got.question_level, 1);
    }

    #[test]
    fn 重复写入为更新() {
        let conn = db_with_words(1);
        upsert(&conn, &state(1, "learning")).unwrap();

        let mut s = state(1, "review");
        s.stability = 60.0;
        s.reps = 5;
        upsert(&conn, &s).unwrap();

        let got = get(&conn, 1).unwrap().unwrap();
        assert_eq!(got.app_state, "review");
        assert_eq!(got.stability, 60.0);
        assert_eq!(got.reps, 5);
        assert_eq!(count_by_app_state(&conn, "review").unwrap(), 1);
        assert_eq!(count_by_app_state(&conn, "learning").unwrap(), 0);
    }

    #[test]
    fn 非法受控值在仓储层被拦截并给出可诊断消息() {
        let conn = db_with_words(1);

        let mut s = state(1, "almost_there");
        let err = upsert(&conn, &s).unwrap_err();
        assert!(err.contains("app_state"), "错误消息应指明是哪个字段: {err}");

        s = state(1, "learning");
        s.fsrs_state = 7;
        assert!(upsert(&conn, &s).unwrap_err().contains("fsrs_state"));

        s = state(1, "learning");
        s.question_level = 0;
        assert!(upsert(&conn, &s).unwrap_err().contains("question_level"));

        s = state(1, "learning");
        s.stability = -1.0;
        assert!(upsert(&conn, &s).is_err());
    }

    #[test]
    fn 引用不存在的词被外键拒绝() {
        let conn = db_with_words(1);
        let result = upsert(&conn, &state(999, "learning"));
        assert!(result.is_err(), "指向不存在词条的状态不应写入成功");
    }

    #[test]
    fn 状态分布与未学习词数正确() {
        let conn = db_with_words(5);
        upsert(&conn, &state(1, "reinforcing")).unwrap();
        upsert(&conn, &state(2, "reinforcing")).unwrap();
        upsert(&conn, &state(3, "review")).unwrap();

        assert_eq!(count_by_app_state(&conn, "reinforcing").unwrap(), 2);
        assert_eq!(count_by_app_state(&conn, "review").unwrap(), 1);
        assert_eq!(untouched_count(&conn, "1=1").unwrap(), 2, "还有 2 个词从未被学习");

        let dist = distribution(&conn).unwrap();
        assert_eq!(dist.len(), 2, "只有两种状态出现过");
    }

    #[test]
    fn 摸底预建但未作答的词仍算未学习() {
        // 这是「已点亮 1589 实则 151」的成因：摸底为一千多个词预建状态行，
        // 按行数算就把它们全算成了已学。判据必须是 reps > 0
        let conn = db_with_words(5);
        let mut seeded = state(1, "review");
        seeded.reps = 0; // 摸底预分级，从未真正作答
        upsert(&conn, &seeded).unwrap();
        upsert(&conn, &state(2, "review")).unwrap(); // reps = 1，真练过

        assert_eq!(
            untouched_count(&conn, "1=1").unwrap(),
            4,
            "预建未答的词不该算作已学"
        );
    }

    #[test]
    fn 未学习统计限定在学习范围内() {
        let conn = db_with_words(5);
        conn.execute("UPDATE words SET level='junior' WHERE id <= 2", []).unwrap();
        conn.execute("UPDATE words SET level='senior' WHERE id > 2", []).unwrap();

        // 高中范围下，初中词不该计入分母
        assert_eq!(untouched_count(&conn, "w.level = 'senior'").unwrap(), 3);
    }

    #[test]
    fn 删除词条级联删除其状态() {
        let conn = db_with_words(1);
        upsert(&conn, &state(1, "learning")).unwrap();
        conn.execute("DELETE FROM words WHERE id = 1", []).unwrap();
        assert!(get(&conn, 1).unwrap().is_none(), "级联删除未生效");
    }
}
