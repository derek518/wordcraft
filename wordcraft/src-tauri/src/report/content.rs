//! 周报内容生成。spec §4.2 F13。
//!
//! 纯逻辑与查询，不含发送——报告内容能被完整测试，SMTP 不能。
//! 两者分开，未验证的部分才不会污染可验证的部分。
//!
//! **收件人是家长，不是学习者。** spec 要求「客户端界面不出现任何相关入口」，
//! 措辞也因此不同：给学习者的文案要鼓励，给家长的要客观——夸大进展会让
//! 家长在孩子实际卡住时毫无察觉。

use rusqlite::Connection;
use serde::Serialize;

/// 报告里列出的顽固词数量。spec：「最顽固 10 个词」。
const STUBBORN_LIMIT: i64 = 10;

#[derive(Debug, Serialize, PartialEq)]
pub struct WeeklyReport {
    pub week_start: String,
    pub week_end: String,
    /// 完成的时段数与应完成数
    pub sessions_done: i64,
    pub sessions_total: i64,
    pub completion_rate: f64,
    /// 本周首次作答的词
    pub new_words: i64,
    /// 本周复习过的词次
    pub reviews: i64,
    pub accuracy: f64,
    /// 词汇量估算的变化
    pub vocab_estimate: i64,
    pub current_streak: i64,
    /// 最顽固的词，按遗忘次数排序
    pub stubborn: Vec<StubbornWord>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct StubbornWord {
    pub word: String,
    pub meaning: String,
    pub lapses: i64,
}

/// 完成率。分母是 21（7 天 × 3 时段）。
pub fn completion_rate(done: i64, total: i64) -> f64 {
    if total <= 0 {
        return 0.0;
    }
    (done as f64 / total as f64).clamp(0.0, 1.0)
}

/// 正确率。无作答时返回 0 而非 NaN——NaN 会让邮件里出现 "NaN%"。
pub fn accuracy(correct: i64, total: i64) -> f64 {
    if total <= 0 {
        return 0.0;
    }
    (correct as f64 / total as f64).clamp(0.0, 1.0)
}

/// 生成纯文本邮件正文。
///
/// 纯文本而非 HTML：家长可能用任何邮件客户端，纯文本一定能读。
pub fn render_text(r: &WeeklyReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("WordCraft 学习周报 {} 至 {}\n", r.week_start, r.week_end));
    out.push_str(&"─".repeat(32));
    out.push('\n');

    out.push_str(&format!(
        "\n完成时段    {}/{}（{:.0}%）\n",
        r.sessions_done,
        r.sessions_total,
        r.completion_rate * 100.0
    ));
    out.push_str(&format!("新学单词    {} 个\n", r.new_words));
    out.push_str(&format!("复习次数    {} 次\n", r.reviews));
    out.push_str(&format!("答题正确率  {:.0}%\n", r.accuracy * 100.0));
    out.push_str(&format!("词汇量估算  {} 词\n", r.vocab_estimate));
    out.push_str(&format!("连续天数    {} 天\n", r.current_streak));

    if r.stubborn.is_empty() {
        out.push_str("\n本周没有反复遗忘的词。\n");
    } else {
        out.push_str("\n需要多留意的词：\n");
        for w in &r.stubborn {
            out.push_str(&format!(
                "  {:<16} {:<20} 忘记 {} 次\n",
                w.word, w.meaning, w.lapses
            ));
        }
    }

    out.push_str("\n本邮件由 WordCraft 自动发送，无需回复。\n");
    out
}

/// 汇总某周的数据。
pub fn build(conn: &Connection, week_start: &str, week_end: &str) -> Result<WeeklyReport, String> {
    use crate::db::repo::player_stats;

    let sessions_done: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sessions
             WHERE is_completed = 1
               AND session_type IN ('morning','noon','evening')
               AND date >= ?1 AND date <= ?2",
            [week_start, week_end],
            |r| r.get(0),
        )
        .map_err(|e| format!("统计时段失败: {e}"))?;

    // 本周首次作答的词：该词的最早一条日志落在本周内
    let new_words: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM (
               SELECT word_id, MIN(date(reviewed_at)) AS first_day
               FROM review_logs GROUP BY word_id
             ) WHERE first_day >= ?1 AND first_day <= ?2",
            [week_start, week_end],
            |r| r.get(0),
        )
        .map_err(|e| format!("统计新学词失败: {e}"))?;

    let (reviews, correct): (i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(is_correct), 0) FROM review_logs
             WHERE date(reviewed_at) >= ?1 AND date(reviewed_at) <= ?2",
            [week_start, week_end],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| format!("统计作答失败: {e}"))?;

    let mut stmt = conn
        .prepare(
            "SELECT w.word, w.meaning, s.lapses
             FROM word_states s JOIN words w ON w.id = s.word_id
             WHERE s.lapses > 0
             ORDER BY s.lapses DESC, w.word
             LIMIT ?1",
        )
        .map_err(|e| format!("准备顽固词查询失败: {e}"))?;

    let stubborn: Vec<StubbornWord> = stmt
        .query_map([STUBBORN_LIMIT], |r| {
            Ok(StubbornWord {
                word: r.get(0)?,
                meaning: r.get(1)?,
                lapses: r.get(2)?,
            })
        })
        .map_err(|e| format!("查询顽固词失败: {e}"))?
        .collect::<Result<_, _>>()
        .map_err(|e| format!("读取顽固词失败: {e}"))?;

    let stats = player_stats::get(conn)?;
    const SESSIONS_TOTAL: i64 = 21;

    Ok(WeeklyReport {
        week_start: week_start.to_string(),
        week_end: week_end.to_string(),
        sessions_done,
        sessions_total: SESSIONS_TOTAL,
        completion_rate: completion_rate(sessions_done, SESSIONS_TOTAL),
        new_words,
        reviews,
        accuracy: accuracy(correct, reviews),
        vocab_estimate: stats.vocab_estimate,
        current_streak: stats.current_streak,
        stubborn,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 零分母不产生_nan() {
        // NaN 会让邮件里出现 "NaN%"，家长看到只会以为程序坏了
        assert_eq!(completion_rate(0, 0), 0.0);
        assert_eq!(accuracy(0, 0), 0.0);
        assert!(!completion_rate(5, 0).is_nan());
        assert!(!accuracy(3, 0).is_nan());
    }

    #[test]
    fn 比率封顶在一() {
        // free 时段可能让完成数超过 21
        assert_eq!(completion_rate(30, 21), 1.0);
        assert_eq!(accuracy(15, 10), 1.0);
    }

    #[test]
    fn 正文包含全部关键指标() {
        let r = WeeklyReport {
            week_start: "2026-08-03".into(),
            week_end: "2026-08-09".into(),
            sessions_done: 15,
            sessions_total: 21,
            completion_rate: 15.0 / 21.0,
            new_words: 42,
            reviews: 310,
            accuracy: 0.87,
            vocab_estimate: 1382,
            current_streak: 5,
            stubborn: vec![StubbornWord {
                word: "acquire".into(),
                meaning: "获得".into(),
                lapses: 4,
            }],
        };
        let text = render_text(&r);

        for expected in ["2026-08-03", "15/21", "42", "310", "87%", "1382", "5 天", "acquire"] {
            assert!(text.contains(expected), "正文缺少 `{expected}`:\n{text}");
        }
    }

    #[test]
    fn 无顽固词时给出明确说明() {
        let r = WeeklyReport {
            week_start: "2026-08-03".into(),
            week_end: "2026-08-09".into(),
            sessions_done: 21,
            sessions_total: 21,
            completion_rate: 1.0,
            new_words: 40,
            reviews: 300,
            accuracy: 0.95,
            vocab_estimate: 1400,
            current_streak: 7,
            stubborn: Vec::new(),
        };
        let text = render_text(&r);
        // 空列表直接省略会让家长以为报告残缺
        assert!(text.contains("没有反复遗忘"), "应明说而非留白:\n{text}");
    }

    /// `build` 的四条 SQL 只有跑起来才会暴露拼写与列名错误。
    /// 建真表、塞真数据、查真结果——这是唯一能挡住的方式。
    mod 对真库查询 {
        use super::*;
        use crate::db::{migrations, repo::word_states, repo::words};

        fn seeded() -> Connection {
            let mut conn = crate::test_support::in_memory_db();
            migrations::run(&mut conn).unwrap();

            let items: Vec<words::WordImport> = (0..4)
                .map(|i| {
                    let w = format!("rep{}", (b'a' + i as u8) as char);
                    words::WordImport {
                        word: w.clone(),
                        phonetic: "/r/".into(),
                        pos: "n.".into(),
                        meaning: format!("释义{i}"),
                        pos_2: None,
                        meaning_2: None,
                        example_1: format!("A {w} here."),
                        example_2: String::new(),
                        level: "senior".into(),
                        frequency_band: 1,
                        frequency_rank: None,
                        zone: "newbie".into(),
                        source_edition: String::new(),
                    }
                })
                .collect();
            let out = words::import(&mut conn, &items).unwrap();
            assert!(out.rejected.is_empty(), "夹具被校验拒收: {:?}", out.rejected);

            // 两个词有遗忘记录，一个没有——顽固词查询必须只挑前者
            for (id, lapses) in [(1_i64, 3_i64), (2, 1), (3, 0)] {
                word_states::upsert(
                    &conn,
                    &word_states::WordState {
                        word_id: id,
                        difficulty: 6.0,
                        stability: 2.0,
                        due_at: crate::db::clock::now(),
                        fsrs_state: 2,
                        app_state: "learning".into(),
                        reps: 4,
                        lapses,
                        question_level: 1,
                        reinforce_streak: 0,
                        last_review_at: None,
                        mastered_at: None,
                    },
                )
                .unwrap();
            }

            // 本周内两条作答（一对一错），上周一条——区间过滤必须排除后者
            conn.execute_batch(
                "INSERT INTO review_logs (word_id, session_id, question_type, is_correct, reaction_ms, rating,
                                          difficulty_before, stability_before, difficulty_after, stability_after, reviewed_at)
                 VALUES (1, NULL, 1, 1, 2000, 3, 6.0, 1.0, 5.8, 2.5, '2026-08-04T02:00:00Z'),
                        (2, NULL, 1, 0, 9000, 1, 6.0, 1.0, 7.2, 0.5, '2026-08-05T02:00:00Z'),
                        (3, NULL, 1, 1, 1500, 3, 6.0, 1.0, 5.8, 2.5, '2026-07-20T02:00:00Z');
                 INSERT INTO sessions (date, session_type, is_completed, planned_count, started_at)
                 VALUES ('2026-08-04', 'morning', 1, 20, '2026-08-04T01:00:00Z'),
                        ('2026-08-05', 'noon',    1, 20, '2026-08-05T05:00:00Z'),
                        ('2026-08-05', 'free',    1, 20, '2026-08-05T09:00:00Z'),
                        ('2026-07-20', 'morning', 1, 20, '2026-07-20T01:00:00Z');",
            )
            .unwrap();
            conn
        }

        #[test]
        fn 只统计区间内且排除自由时段() {
            let conn = seeded();
            let r = build(&conn, "2026-08-03", "2026-08-09").unwrap();

            // free 不是定时时段，计入会让主动多练的人看起来完成度虚高
            assert_eq!(r.sessions_done, 2, "应排除 free 与上周的时段");
            assert_eq!(r.reviews, 2, "上周那条作答不该计入");
            assert_eq!(r.accuracy, 0.5);
        }

        #[test]
        fn 新学词按首次作答日归属() {
            let conn = seeded();
            let r = build(&conn, "2026-08-03", "2026-08-09").unwrap();
            // 词 3 首次作答在上周，本周没碰——不算本周新学
            assert_eq!(r.new_words, 2);
        }

        #[test]
        fn 顽固词按遗忘次数降序且排除零遗忘() {
            let conn = seeded();
            let r = build(&conn, "2026-08-03", "2026-08-09").unwrap();

            assert_eq!(r.stubborn.len(), 2, "lapses=0 的词不该出现");
            assert_eq!(r.stubborn[0].lapses, 3, "最顽固的排最前");
            assert!(r.stubborn[0].word.starts_with("rep"));
            assert!(r.stubborn[0].meaning.starts_with("释义"), "释义要跟着词一起取出");
        }

        #[test]
        fn 空区间产出全零报告而非报错() {
            let conn = seeded();
            // 一个完全没学的周，报告本身仍要能生成——那正是家长该看到的信号
            let r = build(&conn, "2026-06-01", "2026-06-07").unwrap();
            assert_eq!(r.sessions_done, 0);
            assert_eq!(r.reviews, 0);
            assert_eq!(r.accuracy, 0.0);
            assert_eq!(r.new_words, 0);

            let text = render_text(&r);
            assert!(!text.contains("NaN"), "全零周不能渲染出 NaN:\n{text}");
        }
    }

    #[test]
    fn 正文为纯文本不含标记() {
        let r = WeeklyReport {
            week_start: "2026-08-03".into(),
            week_end: "2026-08-09".into(),
            sessions_done: 0,
            sessions_total: 21,
            completion_rate: 0.0,
            new_words: 0,
            reviews: 0,
            accuracy: 0.0,
            vocab_estimate: 0,
            current_streak: 0,
            stubborn: Vec::new(),
        };
        let text = render_text(&r);
        // 家长可能用任何邮件客户端，纯文本一定能读
        assert!(!text.contains('<'), "正文不该含 HTML 标签");
        assert!(text.contains("0%"), "零完成率也要显式呈现");
    }
}
