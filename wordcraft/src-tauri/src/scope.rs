//! 学习范围。决定「哪些词该教」。
//!
//! ## 为什么需要它
//!
//! 词库是完整的高考考纲（junior 1581 + senior 2076）。但一个高中生**不需要
//! 背 `the` / `be` / `in` / `I` / `you`**——他早就会了，把这些排进队列是在
//! 消耗他本就稀缺的时间。
//!
//! 实测数据（2026-08-26 用户实机）：作答 367 次、151 个词，其中 **135 个是
//! band 1**，senior 词只碰过 **8 个**。队列从最常见的词开始发，于是几个月都
//! 走不出初中虚词。
//!
//! ## 为什么用 level 而不是虚词屏蔽名单
//!
//! 词库里 102 个封闭词类（冠词/助动词/代词/介词/连词）有 **96 个是 junior**。
//! 剩下 6 个 senior 的是 `per` / `onto` / `beneath` / `via`——**恰恰是该教的**。
//!
//! 所以按 level 过滤这一个机制就够了，不必再维护一份虚词名单。两个机制会
//! 在边界上互相打架，而且名单一定会漏。

use rusqlite::Connection;

/// 学习范围。存于 `settings.study_level`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StudyLevel {
    /// 初中：学 junior，senior 不出现
    Junior,
    /// 高中（默认）：学 senior。junior 视为已掌握，不再教
    Senior,
    /// 全部：考纲全收。给基础特别薄弱、或想从头过一遍的用户
    All,
}

pub const SETTING_KEY: &str = "study_level";

impl StudyLevel {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "junior" => Some(Self::Junior),
            "senior" => Some(Self::Senior),
            "all" => Some(Self::All),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Junior => "junior",
            Self::Senior => "senior",
            Self::All => "all",
        }
    }

    /// 可以直接拼进 SQL 的 `words` 表条件。
    ///
    /// 返回字面量而非绑定参数：取值来自本枚举，不存在注入面，
    /// 而拼字符串能让这个条件复用在各处已有的 `where_clause` 里。
    pub fn sql_filter(self) -> &'static str {
        match self {
            Self::Junior => "w.level = 'junior'",
            Self::Senior => "w.level = 'senior'",
            Self::All => "1=1",
        }
    }
}

/// 读取当前学习范围。
///
/// 默认 `Senior`：产品面向高中生备考高考。值非法时也回落到默认并记 warn——
/// 学习范围读错的后果是「几个月都在背虚词」，不该静默发生。
pub fn current(conn: &Connection) -> Result<StudyLevel, String> {
    use crate::db::repo::settings;

    let raw = settings::get(conn, SETTING_KEY)?;
    match raw.as_deref() {
        None => Ok(StudyLevel::Senior),
        Some(v) => match StudyLevel::parse(v) {
            Some(level) => Ok(level),
            None => {
                log::warn!("settings.{SETTING_KEY} 的值 `{v}` 无法识别，按 senior 处理");
                Ok(StudyLevel::Senior)
            }
        },
    }
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

    #[test]
    fn 默认高中() {
        // 产品面向高考备考。默认错了，用户要几个月后才会察觉
        assert_eq!(current(&db()).unwrap(), StudyLevel::Senior);
    }

    #[test]
    fn 三种取值都能往返() {
        for lv in [StudyLevel::Junior, StudyLevel::Senior, StudyLevel::All] {
            assert_eq!(StudyLevel::parse(lv.as_str()), Some(lv));
        }
    }

    #[test]
    fn 非法值回落默认而非报错() {
        let conn = db();
        crate::db::repo::settings::set(&conn, SETTING_KEY, "大学").unwrap();
        // 拦不住的坏值不该让整个排队失败——降级到默认，日志留痕
        assert_eq!(current(&conn).unwrap(), StudyLevel::Senior);
    }

    #[test]
    fn 高中范围排除初中虚词() {
        // 这条是整个设计的支点：level 过滤必须真的把 the/be/I 挡在外面
        let mut conn = db();
        crate::db::repo::words::import(
            &mut conn,
            &[
                word("the", "art.", "junior"),
                word("you", "pron.", "junior"),
                word("via", "prep.", "senior"),
                word("subsequent", "adj.", "senior"),
            ],
        )
        .unwrap();

        let sql = format!(
            "SELECT w.word FROM words w WHERE {} ORDER BY w.word",
            StudyLevel::Senior.sql_filter()
        );
        let mut stmt = conn.prepare(&sql).unwrap();
        let got: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(got, vec!["subsequent", "via"], "高中范围应只剩 senior 词");
    }

    #[test]
    fn 初中范围只给初中词() {
        let mut conn = db();
        crate::db::repo::words::import(
            &mut conn,
            &[word("apple", "n.", "junior"), word("subsequent", "adj.", "senior")],
        )
        .unwrap();

        let sql = format!(
            "SELECT COUNT(*) FROM words w WHERE {}",
            StudyLevel::Junior.sql_filter()
        );
        let n: i64 = conn.query_row(&sql, [], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
    }

    fn word(w: &str, pos: &str, level: &str) -> crate::db::repo::words::WordImport {
        crate::db::repo::words::WordImport {
            word: w.into(),
            phonetic: "/w/".into(),
            pos: pos.into(),
            meaning: "释义".into(),
            example_1: format!("A sentence with {w} inside."),
            example_2: String::new(),
            level: level.into(),
            frequency_band: 1,
            zone: "newbie".into(),
            source_edition: String::new(),
        }
    }
}
