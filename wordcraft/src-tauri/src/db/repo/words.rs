//! 词库读写与导入校验。契约见 contracts-v1.md §8。

use crate::db::clock;
use rusqlite::{Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

const VALID_LEVELS: [&str; 3] = ["junior", "senior", "art"];
const VALID_ZONES: [&str; 7] = [
    "newbie", "grass", "water", "fire", "thunder", "ice", "rock",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Word {
    pub id: i64,
    pub word: String,
    pub phonetic: String,
    pub pos: String,
    pub meaning: String,
    pub example_1: String,
    pub example_2: String,
    pub level: String,
    pub frequency_band: i64,
    pub zone: String,
}

/// 导入载荷。字段与 `Word` 分离——导入时无 id，且需经校验才可入库。
#[derive(Debug, Clone, Deserialize)]
pub struct WordImport {
    pub word: String,
    #[serde(default)]
    pub phonetic: String,
    pub pos: String,
    pub meaning: String,
    pub example_1: String,
    #[serde(default)]
    pub example_2: String,
    pub level: String,
    pub frequency_band: i64,
    pub zone: String,
    #[serde(default)]
    pub source_edition: String,
}

#[derive(Debug, Serialize)]
pub struct ImportOutcome {
    pub inserted: i64,
    pub updated: i64,
    pub rejected: Vec<RejectedWord>,
}

#[derive(Debug, Serialize)]
pub struct RejectedWord {
    pub word: String,
    pub reason: String,
}

fn row_to_word(row: &Row) -> rusqlite::Result<Word> {
    Ok(Word {
        id: row.get("id")?,
        word: row.get("word")?,
        phonetic: row.get("phonetic")?,
        pos: row.get("pos")?,
        meaning: row.get("meaning")?,
        example_1: row.get("example_1")?,
        example_2: row.get("example_2")?,
        level: row.get("level")?,
        frequency_band: row.get("frequency_band")?,
        zone: row.get("zone")?,
    })
}

const SELECT_COLS: &str = "id, word, phonetic, pos, meaning, example_1, example_2, \
                           level, frequency_band, zone";

pub fn find_by_id(conn: &Connection, id: i64) -> Result<Option<Word>, String> {
    conn.query_row(
        &format!("SELECT {SELECT_COLS} FROM words WHERE id = ?1"),
        [id],
        row_to_word,
    )
    .optional()
    .map_err(|e| format!("查询词条 {id} 失败: {e}"))
}

pub fn count(conn: &Connection) -> Result<i64, String> {
    conn.query_row("SELECT COUNT(*) FROM words", [], |r| r.get(0))
        .map_err(|e| format!("统计词条数失败: {e}"))
}

pub fn search(conn: &Connection, keyword: &str, limit: i64) -> Result<Vec<Word>, String> {
    let pattern = format!("%{keyword}%");
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {SELECT_COLS} FROM words
             WHERE word LIKE ?1 OR meaning LIKE ?1
             ORDER BY frequency_band, word LIMIT ?2"
        ))
        .map_err(|e| format!("准备搜索语句失败: {e}"))?;

    let rows = stmt
        .query_map(rusqlite::params![pattern, limit], row_to_word)
        .map_err(|e| format!("搜索词条失败: {e}"))?;

    rows.collect::<Result<_, _>>()
        .map_err(|e| format!("读取搜索结果失败: {e}"))
}

/// 干扰项候选池：同词性、排除自身。契约 §6。
///
/// 仅返回候选，具体挑选规则（编辑距离、频段匹配、子串包含检查）由前端
/// `src/core/distractors.ts` 负责——那是纯逻辑，放在能被充分单测的一侧。
pub fn distractor_pool(
    conn: &Connection,
    word_id: i64,
    pos: &str,
    limit: i64,
) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT meaning FROM words
             WHERE pos = ?1 AND id != ?2
             ORDER BY RANDOM() LIMIT ?3",
        )
        .map_err(|e| format!("准备干扰项查询失败: {e}"))?;

    let rows = stmt
        .query_map(rusqlite::params![pos, word_id, limit], |r| {
            r.get::<_, String>(0)
        })
        .map_err(|e| format!("查询干扰项失败: {e}"))?;

    rows.collect::<Result<_, _>>()
        .map_err(|e| format!("读取干扰项失败: {e}"))
}

/// 校验单条导入数据。契约 §8「导入校验」。
///
/// 返回 `Err` 即拒绝该条——**不静默跳过**，拒绝原因会回传给调用方。
pub fn validate(item: &WordImport) -> Result<(), String> {
    let w = item.word.trim();
    if w.is_empty() {
        return Err("单词为空".into());
    }
    if !w
        .chars()
        .all(|c| c.is_ascii_lowercase() || c == '-' || c == '\'' || c == ' ')
        || !w.starts_with(|c: char| c.is_ascii_lowercase())
    {
        return Err(format!("单词 `{w}` 不符合规范（应为小写英文，允许连字符/撇号/空格）"));
    }
    // 音标可缺省，但一旦提供就必须是 /.../ 形式
    let phonetic_ok = item.phonetic.is_empty()
        || (item.phonetic.starts_with('/') && item.phonetic.ends_with('/'));
    if !phonetic_ok {
        return Err(format!("音标 `{}` 未以斜杠包裹", item.phonetic));
    }
    if item.pos.trim().is_empty() {
        return Err("词性为空".into());
    }
    if item.meaning.trim().is_empty() {
        return Err("释义为空".into());
    }
    // 释义含英文字母通常意味着字段错位（把英文塞进了中文列）
    if item.meaning.chars().any(|c| c.is_ascii_alphabetic()) {
        return Err(format!("释义 `{}` 含英文字母，疑似字段错位", item.meaning));
    }
    if item.example_1.trim().is_empty() {
        return Err("例句 example_1 为空".into());
    }
    // 例句必须真的包含该词，否则等于没有例句
    let stem: String = w.chars().take(w.len().saturating_sub(2).max(3)).collect();
    if !item.example_1.to_lowercase().contains(&stem) {
        return Err(format!("例句未包含单词 `{w}`"));
    }
    if !(1..=5).contains(&item.frequency_band) {
        return Err(format!("frequency_band {} 越界（应为 1-5）", item.frequency_band));
    }
    if !VALID_LEVELS.contains(&item.level.as_str()) {
        return Err(format!("level `{}` 不在受控词表中", item.level));
    }
    if !VALID_ZONES.contains(&item.zone.as_str()) {
        return Err(format!("zone `{}` 不在受控词表中", item.zone));
    }
    Ok(())
}

/// 批量导入。校验失败的条目被拒绝并记录原因，不影响其余条目。
///
/// 整批在单个事务中执行：要么全部生效，要么全部回滚。
pub fn import(conn: &mut Connection, items: &[WordImport]) -> Result<ImportOutcome, String> {
    let now = clock::now();
    let mut outcome = ImportOutcome {
        inserted: 0,
        updated: 0,
        rejected: Vec::new(),
    };

    let tx = conn
        .transaction()
        .map_err(|e| format!("开启导入事务失败: {e}"))?;

    for item in items {
        if let Err(reason) = validate(item) {
            outcome.rejected.push(RejectedWord {
                word: item.word.clone(),
                reason,
            });
            continue;
        }

        let existed: bool = tx
            .query_row(
                "SELECT 1 FROM words WHERE word = ?1",
                [&item.word],
                |_| Ok(()),
            )
            .optional()
            .map_err(|e| format!("查询词条 `{}` 是否存在失败: {e}", item.word))?
            .is_some();

        tx.execute(
            "INSERT INTO words
               (word, phonetic, pos, meaning, example_1, example_2,
                level, frequency_band, zone, source_edition, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(word) DO UPDATE SET
               phonetic = excluded.phonetic,
               pos = excluded.pos,
               meaning = excluded.meaning,
               example_1 = excluded.example_1,
               example_2 = excluded.example_2,
               level = excluded.level,
               frequency_band = excluded.frequency_band,
               zone = excluded.zone,
               source_edition = excluded.source_edition",
            rusqlite::params![
                item.word,
                item.phonetic,
                item.pos,
                item.meaning,
                item.example_1,
                item.example_2,
                item.level,
                item.frequency_band,
                item.zone,
                item.source_edition,
                now,
            ],
        )
        .map_err(|e| format!("写入词条 `{}` 失败: {e}", item.word))?;

        if existed {
            outcome.updated += 1;
        } else {
            outcome.inserted += 1;
        }
    }

    tx.commit().map_err(|e| format!("提交导入事务失败: {e}"))?;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;
    use crate::test_support::in_memory_db;

    fn db() -> Connection {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();
        conn
    }

    fn sample(word: &str) -> WordImport {
        WordImport {
            word: word.into(),
            phonetic: "/ˈkrɪstl/".into(),
            pos: "n.".into(),
            meaning: "水晶".into(),
            example_1: format!("A glowing {word} lights the cave."),
            example_2: String::new(),
            level: "junior".into(),
            frequency_band: 1,
            zone: "newbie".into(),
            source_edition: "renjiao".into(),
        }
    }

    #[test]
    fn 合法词条通过校验并入库() {
        let mut conn = db();
        let outcome = import(&mut conn, &[sample("crystal")]).unwrap();
        assert_eq!(outcome.inserted, 1);
        assert_eq!(outcome.updated, 0);
        assert!(outcome.rejected.is_empty());
        assert_eq!(count(&conn).unwrap(), 1);
    }

    #[test]
    fn 重复导入同一词为更新而非新增() {
        let mut conn = db();
        import(&mut conn, &[sample("crystal")]).unwrap();

        let mut changed = sample("crystal");
        changed.meaning = "结晶体".into();
        let outcome = import(&mut conn, &[changed]).unwrap();

        assert_eq!(outcome.inserted, 0);
        assert_eq!(outcome.updated, 1);
        assert_eq!(count(&conn).unwrap(), 1);

        let w = find_by_id(&conn, 1).unwrap().unwrap();
        assert_eq!(w.meaning, "结晶体", "更新未生效");
    }

    #[test]
    fn 非法词条被拒绝且附带原因() {
        let mut conn = db();

        let mut bad_meaning = sample("alpha");
        bad_meaning.meaning = "crystal".into(); // 释义写成英文 = 字段错位

        let mut bad_band = sample("beta");
        bad_band.frequency_band = 9;

        let mut bad_zone = sample("gamma");
        bad_zone.zone = "nowhere".into();

        let mut bad_example = sample("delta");
        bad_example.example_1 = "This sentence lacks the word.".into();

        let outcome = import(
            &mut conn,
            &[bad_meaning, bad_band, bad_zone, bad_example, sample("crystal")],
        )
        .unwrap();

        assert_eq!(outcome.inserted, 1, "只有合法的那条应入库");
        assert_eq!(outcome.rejected.len(), 4);
        for r in &outcome.rejected {
            assert!(!r.reason.is_empty(), "拒绝原因不能为空");
        }
        assert_eq!(count(&conn).unwrap(), 1);
    }

    #[test]
    fn 校验规则逐条生效() {
        let mut w = sample("crystal");
        assert!(validate(&w).is_ok());

        w.word = "Crystal".into();
        assert!(validate(&w).is_err(), "大写未被拒绝");

        w = sample("crystal");
        w.phonetic = "krɪstl".into();
        assert!(validate(&w).is_err(), "缺斜杠的音标未被拒绝");

        w = sample("crystal");
        w.meaning = "".into();
        assert!(validate(&w).is_err(), "空释义未被拒绝");

        w = sample("crystal");
        w.level = "college".into();
        assert!(validate(&w).is_err(), "非法 level 未被拒绝");
    }

    #[test]
    fn 干扰项池排除自身且限定同词性() {
        let mut conn = db();
        let mut items: Vec<WordImport> = (0..5)
            .map(|i| {
                let mut w = sample(&format!("noun{}", (b'a' + i) as char));
                w.meaning = format!("名词释义{i}");
                w
            })
            .collect();
        let mut verb = sample("run");
        verb.pos = "v.".into();
        verb.meaning = "跑".into();
        items.push(verb);
        import(&mut conn, &items).unwrap();

        let pool = distractor_pool(&conn, 1, "n.", 10).unwrap();
        assert_eq!(pool.len(), 4, "应返回同词性的其余 4 个");
        assert!(!pool.contains(&"名词释义0".to_string()), "自身不应出现在干扰项中");
        assert!(!pool.contains(&"跑".to_string()), "不同词性不应出现");
    }

    #[test]
    fn 搜索命中单词与释义() {
        let mut conn = db();
        import(&mut conn, &[sample("crystal")]).unwrap();

        assert_eq!(search(&conn, "cryst", 10).unwrap().len(), 1);
        assert_eq!(search(&conn, "水晶", 10).unwrap().len(), 1);
        assert_eq!(search(&conn, "不存在的词", 10).unwrap().len(), 0);
    }

    #[test]
    fn 空库查询返回空而非报错() {
        let conn = db();
        assert_eq!(count(&conn).unwrap(), 0);
        assert!(find_by_id(&conn, 1).unwrap().is_none());
        assert!(search(&conn, "x", 10).unwrap().is_empty());
        assert!(distractor_pool(&conn, 1, "n.", 3).unwrap().is_empty());
    }
}
