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
///
/// **这是可选的考纲约束，不是难度选择器。** 难度由能力模型决定（见
/// `ability.rs` 与契约 §10）——按 `level` 标签筛难度本来就不成立：
/// 102 个高中词的常用度和 `the` 同级，28 个初中词比大多数四级词还生僻。
///
/// 留着它是因为「只想过一遍高考考纲」是个正当诉求。默认 `All`：
/// 让能力模型在全库里挑，那才是它该干的事。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StudyLevel {
    /// 只练中考考纲
    Junior,
    /// 只练高考考纲
    Senior,
    /// 只练四级词
    Cet4,
    /// 全库（默认）：不设考纲限制，由能力模型挑词
    All,
}

pub const SETTING_KEY: &str = "study_level";

impl StudyLevel {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "junior" => Some(Self::Junior),
            "senior" => Some(Self::Senior),
            "cet4" => Some(Self::Cet4),
            "all" => Some(Self::All),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Junior => "junior",
            Self::Senior => "senior",
            Self::Cet4 => "cet4",
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
            Self::Cet4 => "w.level = 'cet4'",
            Self::All => "1=1",
        }
    }
}

/// 读取当前学习范围。
///
/// 默认 `All`。
///
/// 先前默认 `Senior`，那是在用考纲标签冒充难度选择器——而标签和难度基本无关。
/// 现在难度由能力模型负责，范围只在用户明确要求「只过考纲」时才该收窄。
///
/// 值非法时回落到默认并记 warn：范围读错的后果是「几个月都在练错误的词」，
/// 不该静默发生。
pub fn current(conn: &Connection) -> Result<StudyLevel, String> {
    use crate::db::repo::settings;

    let raw = settings::get(conn, SETTING_KEY)?;
    match raw.as_deref() {
        None => Ok(StudyLevel::All),
        Some(v) => match StudyLevel::parse(v) {
            Some(level) => Ok(level),
            None => {
                log::warn!("settings.{SETTING_KEY} 的值 `{v}` 无法识别，按 all 处理");
                Ok(StudyLevel::All)
            }
        },
    }
}

/// 各范围在库中的真实词数。
///
/// 界面上的选项由它渲染，而不是写死一组数字。这个项目已经三次栽在写死的
/// 计数上：蓝图块数、赛道积分、词库总数——词库一变就成了谎话。
/// 顺带，四级词导入后 `cet4` 选项会自动出现，不必再改一次前端。
#[derive(Debug, serde::Serialize)]
pub struct LevelOption {
    pub value: String,
    pub label: String,
    pub words: i64,
}

pub fn options(conn: &Connection) -> Result<Vec<LevelOption>, String> {
    let mut stmt = conn
        .prepare("SELECT level, COUNT(*) FROM words GROUP BY level")
        .map_err(|e| format!("准备范围统计失败: {e}"))?;
    let rows: Vec<(String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| format!("统计各范围词数失败: {e}"))?
        .collect::<Result<_, _>>()
        .map_err(|e| format!("读取范围统计失败: {e}"))?;

    fn label_of(lv: &str) -> &str {
        match lv {
            "junior" => "初中",
            "senior" => "高中",
            "cet4" => "四级",
            other => other,
        }
    }

    let total: i64 = rows.iter().map(|(_, n)| n).sum();
    // 只列出库里真有词的范围。给出一个点了会得到空队列的选项，
    // 比不给这个选项更糟
    let mut out: Vec<LevelOption> = rows
        .iter()
        .filter(|(_, n)| *n > 0)
        .map(|(lv, n)| LevelOption {
            value: lv.clone(),
            label: label_of(lv).to_string(),
            words: *n,
        })
        .collect();
    out.sort_by_key(|o| match o.value.as_str() {
        "junior" => 0,
        "senior" => 1,
        "cet4" => 2,
        _ => 3,
    });
    if out.len() > 1 {
        out.push(LevelOption {
            value: "all".into(),
            label: "全部".into(),
            words: total,
        });
    }
    Ok(out)
}

/// 启动时报告当前学习范围。
///
/// 这次「几个月都在背 the」之所以难以察觉，正是因为没有任何地方说得出
/// 「现在在教哪一批词」。一行日志的成本，抵得上一次几百条作答的浪费。
pub fn log_current(conn: &Connection) {
    match current(conn) {
        Ok(level) => log::info!("学习范围: {}", level.as_str()),
        Err(e) => log::warn!("读取学习范围失败: {e}"),
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
    fn 默认全库() {
        // 范围是可选的考纲约束，不是难度选择器——难度由能力模型负责。
        // 默认收窄到某个考纲，等于替用户猜他孩子什么水平，而那正是
        // 这套设计要消除的东西
        assert_eq!(current(&db()).unwrap(), StudyLevel::All);
    }

    #[test]
    fn 三种取值都能往返() {
        for lv in [StudyLevel::Junior, StudyLevel::Senior, StudyLevel::Cet4, StudyLevel::All] {
            assert_eq!(StudyLevel::parse(lv.as_str()), Some(lv));
        }
    }

    #[test]
    fn 非法值回落默认而非报错() {
        let conn = db();
        crate::db::repo::settings::set(&conn, SETTING_KEY, "大学").unwrap();
        // 拦不住的坏值不该让整个排队失败——降级到默认，日志留痕
        assert_eq!(current(&conn).unwrap(), StudyLevel::All);
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
    fn 选项由库中真实词数导出() {
        let mut conn = db();
        crate::db::repo::words::import(
            &mut conn,
            &[word("apple", "n.", "junior"), word("via", "prep.", "senior"),
              word("subsequent", "adj.", "senior")],
        )
        .unwrap();

        let opts = options(&conn).unwrap();
        let junior = opts.iter().find(|o| o.value == "junior").unwrap();
        let senior = opts.iter().find(|o| o.value == "senior").unwrap();
        // 写死的数字会在词库更新后成为谎话——本项目已经三次栽在这上面
        assert_eq!(junior.words, 1);
        assert_eq!(senior.words, 2);
        assert_eq!(opts.iter().find(|o| o.value == "all").unwrap().words, 3);
    }

    #[test]
    fn 库里没有的范围不出现在选项里() {
        let mut conn = db();
        crate::db::repo::words::import(&mut conn, &[word("apple", "n.", "junior")]).unwrap();

        let opts = options(&conn).unwrap();
        // 给出一个点了会得到空队列的选项，比不给这个选项更糟。
        // 四级词导入后 cet4 会自动出现，不必再改一次前端
        assert!(!opts.iter().any(|o| o.value == "cet4"));
        assert!(!opts.iter().any(|o| o.value == "senior"));
        assert!(!opts.iter().any(|o| o.value == "all"), "只有一档时不必再给「全部」");
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
            frequency_rank: None,
            zone: "newbie".into(),
            source_edition: String::new(),
        }
    }
}
