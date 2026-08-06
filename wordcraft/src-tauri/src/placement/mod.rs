//! 摸底分级。contracts §9.2。
//!
//! 目标是**压缩实际待学量**，不是给每个词打标签——60 题覆盖不了 1600 词，
//! 产出的是每层掌握率而非逐词判定。判错的词由 §9.2④ 的日常抽查纠正。

mod grading;

pub use grading::{
    estimate_vocab, grade_for, is_pass, stability_range, PreGrade, CONSECUTIVE_MISS_LIMIT,
    PLACEMENT_LEVEL, QUESTIONS_PER_BAND,
};

use crate::db::{clock, repo::settings, repo::word_states, Db};
use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

/// 频段层数。
const BANDS: [i64; 5] = [1, 2, 3, 4, 5];

#[derive(Debug, Serialize)]
pub struct PlacementQuestion {
    pub word_id: i64,
    pub word: String,
    pub phonetic: String,
    pub pos: String,
    pub meaning: String,
    pub band: i64,
    /// 已答题数与预计总题数，用于渲染进度
    pub answered: i64,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct PlacementOutcome {
    pub vocab_estimate: i64,
    /// 各层掌握率，按 band 升序
    pub pass_rates: Vec<f64>,
    /// 预分级影响的词数，按状态分类
    pub graded_review: i64,
    pub graded_learning: i64,
    pub skipped_new: i64,
}

fn lock(db: &Db) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
    db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))
}

/// 当前应出题的频段：第一个未测完的层。
///
/// 返回 None 表示所有层都已关闭，摸底可以结算了。
fn active_band(conn: &Connection) -> Result<Option<i64>, String> {
    for band in BANDS {
        let closed: Option<i64> = conn
            .query_row(
                "SELECT is_closed FROM placement_results WHERE band = ?1",
                [band],
                |r| r.get(0),
            )
            .ok();
        if closed != Some(1) {
            return Ok(Some(band));
        }
    }
    Ok(None)
}

/// 取一道未出过的题。
fn pick_question(conn: &Connection, band: i64) -> Result<Option<PlacementQuestion>, String> {
    let (answered, total) = progress(conn)?;

    let row = conn
        .query_row(
            "SELECT w.id, w.word, w.phonetic, w.pos, w.meaning
             FROM words w
             WHERE w.level = ?1 AND w.frequency_band = ?2
               AND NOT EXISTS (SELECT 1 FROM placement_asked a WHERE a.word_id = w.id)
             ORDER BY RANDOM() LIMIT 1",
            rusqlite::params![PLACEMENT_LEVEL, band],
            |r| {
                Ok(PlacementQuestion {
                    word_id: r.get(0)?,
                    word: r.get(1)?,
                    phonetic: r.get(2)?,
                    pos: r.get(3)?,
                    meaning: r.get(4)?,
                    band,
                    answered,
                    total,
                })
            },
        )
        .ok();

    Ok(row)
}

fn progress(conn: &Connection) -> Result<(i64, i64), String> {
    let answered: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(asked), 0) FROM placement_results",
            [],
            |r| r.get(0),
        )
        .map_err(|e| format!("统计摸底进度失败: {e}"))?;
    Ok((answered, BANDS.len() as i64 * QUESTIONS_PER_BAND))
}

/// 关闭一层，并在连续答错时跳过所有更难的层。
///
/// 更难的层必然更不会——继续测只是浪费用户时间。契约 §9.2② 的
/// 「连续 3 题错则下跳」在此实现为提前收束整个摸底。
fn close_band(conn: &Connection, band: i64, skip_harder: bool) -> Result<(), String> {
    // 必须 INSERT..ON CONFLICT 而非 UPDATE：一题未答的层还没有行，
    // UPDATE 影响 0 行且不报错，关闭动作静默失效——而连错跳过恰恰
    // 总是作用在这种从未答过题的层上
    let close_one = |b: i64| -> Result<(), String> {
        conn.execute(
            "INSERT INTO placement_results (band, asked, passed, is_closed)
             VALUES (?1, 0, 0, 1)
             ON CONFLICT(band) DO UPDATE SET is_closed = 1",
            [b],
        )
        .map_err(|e| format!("关闭频段 {b} 失败: {e}"))?;
        Ok(())
    };

    close_one(band)?;

    if skip_harder {
        for b in BANDS.iter().copied().filter(|b| *b > band) {
            close_one(b)?;
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────
// Commands
// ─────────────────────────────────────────────

/// contracts §3.6：取下一道摸底题。返回 None 表示摸底已结束。
#[tauri::command]
pub fn get_placement_question(db: State<Db>) -> Result<Option<PlacementQuestion>, String> {
    let conn = lock(&db)?;

    settings::set(&conn, "placement_stage", "1")?;

    let Some(band) = active_band(&conn)? else {
        return Ok(None);
    };

    match pick_question(&conn, band)? {
        Some(q) => Ok(Some(q)),
        None => {
            // 该层的词已出完却还没关闭——词库中此层词数不足 12。
            // 关掉它继续下一层，而不是卡在这里返回空
            close_band(&conn, band, false)?;
            let next = active_band(&conn)?;
            match next {
                Some(b) => pick_question(&conn, b),
                None => Ok(None),
            }
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AnswerOutcome {
    /// 该层是否就此结束（题量满或连错触发）
    pub band_closed: bool,
    /// 整个摸底是否已结束，可以调 finalize 了
    pub placement_done: bool,
}

/// 提交一题的作答。
///
/// 收束规则完全在后端判定，返回值只告诉前端「这层结束了没有」。
/// 把「连错 3 次」这类阈值暴露给前端，等于让同一条产品规则活在两个地方。
#[tauri::command]
pub fn submit_placement_answer(
    db: State<Db>,
    word_id: i64,
    band: i64,
    is_correct: bool,
    reaction_ms: i64,
) -> Result<AnswerOutcome, String> {
    if !BANDS.contains(&band) {
        return Err(format!("band 必须在 1..5，收到 {band}"));
    }
    if reaction_ms < 0 {
        return Err(format!("reaction_ms 不能为负，收到 {reaction_ms}"));
    }

    let conn = lock(&db)?;
    let passed = i64::from(is_pass(is_correct, reaction_ms));
    // 连错计数按「是否答对」而非「是否算已会」累加：答对但超时说明还有印象，
    // 不该和完全不认识同等对待
    let miss_delta = i64::from(!is_correct);

    conn.execute(
        "INSERT INTO placement_asked (word_id, asked_at) VALUES (?1, ?2)
         ON CONFLICT(word_id) DO NOTHING",
        rusqlite::params![word_id, clock::now()],
    )
    .map_err(|e| format!("记录已出题失败: {e}"))?;

    conn.execute(
        "INSERT INTO placement_results (band, asked, passed, consecutive_miss)
         VALUES (?1, 1, ?2, ?3)
         ON CONFLICT(band) DO UPDATE SET
           asked = asked + 1,
           passed = passed + ?2,
           -- 答对即清零：连错要求的是「连续」
           consecutive_miss = CASE WHEN ?3 = 1 THEN consecutive_miss + 1 ELSE 0 END",
        rusqlite::params![band, passed, miss_delta],
    )
    .map_err(|e| format!("更新摸底统计失败: {e}"))?;

    let (asked, misses): (i64, i64) = conn
        .query_row(
            "SELECT asked, consecutive_miss FROM placement_results WHERE band = ?1",
            [band],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| format!("读取摸底统计失败: {e}"))?;

    // 连错达上限：该层已超出能力，且更难的层必然更不会——一并跳过
    let aborted = misses >= CONSECUTIVE_MISS_LIMIT;
    let filled = asked >= QUESTIONS_PER_BAND;

    if aborted || filled {
        close_band(&conn, band, aborted)?;
    }

    Ok(AnswerOutcome {
        band_closed: aborted || filled,
        placement_done: active_band(&conn)?.is_none(),
    })
}

/// contracts §3.6：结算摸底并批量预分级。
#[tauri::command]
pub fn finalize_placement(db: State<Db>) -> Result<PlacementOutcome, String> {
    let mut conn = db
        .0
        .lock()
        .map_err(|e| format!("获取数据库锁失败: {e}"))?;

    // 各层掌握率
    let mut pass_rates: Vec<(i64, f64)> = Vec::new();
    for band in BANDS {
        let row: Option<(i64, i64)> = conn
            .query_row(
                "SELECT asked, passed FROM placement_results WHERE band = ?1",
                [band],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok();
        let rate = match row {
            Some((asked, passed)) if asked > 0 => passed as f64 / asked as f64,
            // 未测的层按 0：没有证据就不算掌握
            _ => 0.0,
        };
        pass_rates.push((band, rate));
    }

    // 各层词数（仅摸底范围）
    let mut band_totals: Vec<(i64, i64)> = Vec::new();
    for band in BANDS {
        let total: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM words WHERE level = ?1 AND frequency_band = ?2",
                rusqlite::params![PLACEMENT_LEVEL, band],
                |r| r.get(0),
            )
            .map_err(|e| format!("统计频段 {band} 词数失败: {e}"))?;
        band_totals.push((band, total));
    }

    let vocab_estimate = estimate_vocab(&band_totals, &pass_rates);

    // 批量预分级必须整体成败：中途失败会留下一半已分级、一半未分级的库，
    // 而 placement_stage 无从表达这种中间态
    let tx = conn
        .transaction()
        .map_err(|e| format!("开启预分级事务失败: {e}"))?;

    let mut graded_review = 0i64;
    let mut graded_learning = 0i64;
    let mut skipped_new = 0i64;
    let mut rng = Lcg(0x9E37_79B9);

    for (band, rate) in &pass_rates {
        let grade = grade_for(*rate);
        if grade == PreGrade::New {
            let n: i64 = tx
                .query_row(
                    "SELECT COUNT(*) FROM words WHERE level = ?1 AND frequency_band = ?2",
                    rusqlite::params![PLACEMENT_LEVEL, band],
                    |r| r.get(0),
                )
                .map_err(|e| format!("统计频段 {band} 失败: {e}"))?;
            skipped_new += n;
            continue;
        }

        let ids: Vec<i64> = {
            let mut stmt = tx
                .prepare(
                    "SELECT w.id FROM words w
                     WHERE w.level = ?1 AND w.frequency_band = ?2
                       AND NOT EXISTS (SELECT 1 FROM word_states s WHERE s.word_id = w.id)",
                )
                .map_err(|e| format!("准备预分级查询失败: {e}"))?;
            let rows = stmt
                .query_map(rusqlite::params![PLACEMENT_LEVEL, band], |r| r.get(0))
                .map_err(|e| format!("查询待分级词失败: {e}"))?;
            rows.collect::<Result<_, _>>()
                .map_err(|e| format!("读取待分级词失败: {e}"))?
        };

        let (lo, hi) = stability_range(*band);
        for id in ids {
            // 逐词抖动而非整层同值——同值会让它们在同一天集中到期
            let stability = lo + rng.next_f64() * (hi - lo);
            let state = word_states::WordState {
                word_id: id,
                difficulty: 5.0,
                stability,
                due_at: clock::due_in_days(stability),
                fsrs_state: if grade == PreGrade::Review { 2 } else { 1 },
                app_state: grade.app_state().to_string(),
                reps: 0,
                lapses: 0,
                question_level: grade.question_level(),
                reinforce_streak: 0,
                last_review_at: None,
                mastered_at: None,
            };
            word_states::upsert(&tx, &state)?;

            match grade {
                PreGrade::Review => graded_review += 1,
                PreGrade::Learning => graded_learning += 1,
                PreGrade::New => {}
            }
        }
    }

    settings::set(&tx, "placement_stage", "2")?;
    crate::db::repo::player_stats::set_vocab_estimate(&tx, vocab_estimate)?;

    tx.commit()
        .map_err(|e| format!("提交预分级事务失败: {e}"))?;

    Ok(PlacementOutcome {
        vocab_estimate,
        pass_rates: pass_rates.iter().map(|(_, r)| *r).collect(),
        graded_review,
        graded_learning,
        skipped_new,
    })
}

/// 线性同余伪随机数。
///
/// 只用于 stability 抖动，无需密码学强度；自带实现避免引入 rand 依赖。
struct Lcg(u64);

impl Lcg {
    fn next_f64(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;
    use crate::db::repo::words;
    use crate::test_support::in_memory_db;

    /// 造词。词形必须是纯字母——契约 §8 的 `^[a-z][a-z\-' ]*$` 会拒掉带数字的，
    /// 用 `w1x0` 这类命名会让整批数据被拒，测试跑在空库上却仍然「通过」。
    fn make_word(prefix: &str, n: usize) -> String {
        let hi = (b'a' + (n / 26) as u8) as char;
        let lo = (b'a' + (n % 26) as u8) as char;
        format!("{prefix}{hi}{lo}")
    }

    fn seed(junior_per_band: usize) -> Connection {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();

        let mut items = Vec::new();
        for band in 1..=5i64 {
            let prefix = format!("jun{}", (b'a' + band as u8 - 1) as char);
            for i in 0..junior_per_band {
                let w = make_word(&prefix, i);
                items.push(words::WordImport {
                    word: w.clone(),
                    phonetic: "/w/".into(),
                    pos: "n.".into(),
                    meaning: format!("初中释义{band}-{i}"),
                    example_1: format!("A {w} appears here."),
                    example_2: String::new(),
                    level: "junior".into(),
                    frequency_band: band,
                    zone: "newbie".into(),
                    source_edition: String::new(),
                });
            }
        }
        // 高中词不参与摸底（§9.2①）
        for i in 0..10 {
            let w = make_word("sen", i);
            items.push(words::WordImport {
                word: w.clone(),
                phonetic: "/w/".into(),
                pos: "n.".into(),
                meaning: format!("高中释义{i}"),
                example_1: format!("A {w} appears here."),
                example_2: String::new(),
                level: "senior".into(),
                frequency_band: 1,
                zone: "grass".into(),
                source_edition: String::new(),
            });
        }

        let outcome = words::import(&mut conn, &items).unwrap();
        assert!(
            outcome.rejected.is_empty(),
            "测试数据未通过契约校验，测试会跑在空库上：{:?}",
            outcome.rejected
        );
        conn
    }

    fn answer(conn: &Connection, band: i64, word_id: i64, pass: bool) {
        let passed = i64::from(pass);
        conn.execute(
            "INSERT INTO placement_asked (word_id, asked_at) VALUES (?1, '2026-08-06T00:00:00Z')
             ON CONFLICT(word_id) DO NOTHING",
            [word_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO placement_results (band, asked, passed) VALUES (?1, 1, ?2)
             ON CONFLICT(band) DO UPDATE SET asked = asked + 1, passed = passed + ?2",
            rusqlite::params![band, passed],
        )
        .unwrap();
    }

    #[test]
    fn 出题只覆盖初中词() {
        let conn = seed(20);
        for band in 1..=5 {
            let q = pick_question(&conn, band).unwrap();
            let q = q.expect("应能取到题");
            assert!(
                q.word.starts_with("jun"),
                "取到了非初中词 {}——§9.2① 要求摸底范围仅 junior",
                q.word
            );
            assert_eq!(q.band, band);
        }
    }

    #[test]
    fn 已出过的词不再重复出现() {
        let conn = seed(2);
        let first = pick_question(&conn, 1).unwrap().unwrap();
        answer(&conn, 1, first.word_id, true);

        let second = pick_question(&conn, 1).unwrap().unwrap();
        assert_ne!(second.word_id, first.word_id);
    }

    #[test]
    fn 该层词不足时返回空而非报错() {
        let conn = seed(1);
        let q = pick_question(&conn, 1).unwrap().unwrap();
        answer(&conn, 1, q.word_id, true);
        assert!(pick_question(&conn, 1).unwrap().is_none());
    }

    #[test]
    fn 连续答错会跳过所有更难的层() {
        let conn = seed(20);
        // 真实路径是从 band 1 顺序往下测，到 band 2 才连错
        close_band(&conn, 1, false).unwrap();
        close_band(&conn, 2, true).unwrap();

        // band 3/4/5 应被一并关闭——更难的层必然更不会，继续测是浪费时间
        for band in 3..=5 {
            let closed: i64 = conn
                .query_row(
                    "SELECT is_closed FROM placement_results WHERE band = ?1",
                    [band],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(closed, 1, "band {band} 未被跳过");
        }
        assert!(active_band(&conn).unwrap().is_none(), "应无待测层");
    }

    #[test]
    fn 连错计数被答对清零() {
        let conn = seed(20);
        let bump = |correct: bool| {
            let miss = i64::from(!correct);
            conn.execute(
                "INSERT INTO placement_results (band, asked, passed, consecutive_miss)
                 VALUES (1, 1, 0, ?1)
                 ON CONFLICT(band) DO UPDATE SET
                   asked = asked + 1,
                   consecutive_miss = CASE WHEN ?1 = 1 THEN consecutive_miss + 1 ELSE 0 END",
                [miss],
            )
            .unwrap();
            conn.query_row(
                "SELECT consecutive_miss FROM placement_results WHERE band = 1",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
        };

        assert_eq!(bump(false), 1);
        assert_eq!(bump(false), 2);
        // 答对必须清零——规则要求的是「连续」答错，不是累计答错
        assert_eq!(bump(true), 0);
        assert_eq!(bump(false), 1);
    }

    #[test]
    fn 关闭单层不影响其他层() {
        let conn = seed(20);
        close_band(&conn, 1, false).unwrap();
        assert_eq!(active_band(&conn).unwrap(), Some(2));
    }

    #[test]
    fn 高掌握率的层被判为已复习并跳过新词队列() {
        let mut conn = seed(20);
        // band 1 全对 → p = 1.0 > 0.85
        for i in 0..12 {
            answer(&conn, 1, i + 1, true);
        }
        for band in 2..=5 {
            close_band(&conn, band, false).unwrap();
        }

        let outcome = finalize_with(&mut conn).unwrap();
        assert!(outcome.graded_review > 0, "band 1 应被判为已掌握");

        // 这些词不应再作为新词排队
        let new_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM words w
                 WHERE w.frequency_band = 1 AND w.level = 'junior'
                   AND NOT EXISTS (SELECT 1 FROM word_states s WHERE s.word_id = w.id)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(new_count, 0, "已判定掌握的词仍会作为新词出现");
    }

    #[test]
    fn 低掌握率的层保持新词状态() {
        let mut conn = seed(20);
        // band 1 全错 → p = 0
        for i in 0..12 {
            answer(&conn, 1, i + 1, false);
        }
        for band in 2..=5 {
            close_band(&conn, band, false).unwrap();
        }

        let outcome = finalize_with(&mut conn).unwrap();
        assert_eq!(outcome.graded_review, 0);
        assert!(outcome.skipped_new > 0, "低掌握率的层应保持新词");
    }

    #[test]
    fn 预分级的到期日分散而非集中() {
        let mut conn = seed(40);
        for i in 0..12 {
            answer(&conn, 1, i + 1, true);
        }
        for band in 2..=5 {
            close_band(&conn, band, false).unwrap();
        }
        finalize_with(&mut conn).unwrap();

        let distinct_days: i64 = conn
            .query_row(
                "SELECT COUNT(DISTINCT substr(due_at, 1, 10)) FROM word_states",
                [],
                |r| r.get(0),
            )
            .unwrap();
        // 若全赋同一 stability，所有词会落在同一天，把每日预算彻底淹没
        assert!(
            distinct_days > 5,
            "到期日只分散到 {distinct_days} 天，抖动未生效"
        );
    }

    #[test]
    fn 结算是原子的且置位完成标记() {
        let mut conn = seed(20);
        for i in 0..12 {
            answer(&conn, 1, i + 1, true);
        }
        for band in 2..=5 {
            close_band(&conn, band, false).unwrap();
        }
        finalize_with(&mut conn).unwrap();

        assert_eq!(
            settings::get(&conn, "placement_stage").unwrap().as_deref(),
            Some("2")
        );
        let est = crate::db::repo::player_stats::get(&conn).unwrap().vocab_estimate;
        assert!(est > 0, "词汇量估算未写入");
    }

    #[test]
    fn 已有学习记录的词不被预分级覆盖() {
        let mut conn = seed(20);
        // 先给一个词造真实学习记录
        word_states::upsert(
            &conn,
            &word_states::WordState {
                word_id: 1,
                difficulty: 7.0,
                stability: 42.0,
                due_at: "2026-09-01T00:00:00Z".into(),
                fsrs_state: 2,
                app_state: "review".into(),
                reps: 9,
                lapses: 2,
                question_level: 4,
                reinforce_streak: 0,
                last_review_at: Some("2026-08-01T00:00:00Z".into()),
                mastered_at: None,
            },
        )
        .unwrap();

        for i in 0..12 {
            answer(&conn, 1, i + 2, true);
        }
        for band in 2..=5 {
            close_band(&conn, band, false).unwrap();
        }
        finalize_with(&mut conn).unwrap();

        // 真实作答积累的进度不该被摸底的估算值抹掉
        let s = word_states::get(&conn, 1).unwrap().unwrap();
        assert_eq!(s.reps, 9, "已有学习记录被预分级覆盖");
        assert_eq!(s.question_level, 4);
    }

    /// 测试用：绕过 tauri::State 直接结算。
    fn finalize_with(conn: &mut Connection) -> Result<PlacementOutcome, String> {
        let mut pass_rates: Vec<(i64, f64)> = Vec::new();
        for band in BANDS {
            let row: Option<(i64, i64)> = conn
                .query_row(
                    "SELECT asked, passed FROM placement_results WHERE band = ?1",
                    [band],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .ok();
            let rate = match row {
                Some((asked, passed)) if asked > 0 => passed as f64 / asked as f64,
                _ => 0.0,
            };
            pass_rates.push((band, rate));
        }

        let mut band_totals: Vec<(i64, i64)> = Vec::new();
        for band in BANDS {
            let total: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM words WHERE level = ?1 AND frequency_band = ?2",
                    rusqlite::params![PLACEMENT_LEVEL, band],
                    |r| r.get(0),
                )
                .unwrap();
            band_totals.push((band, total));
        }
        let vocab_estimate = estimate_vocab(&band_totals, &pass_rates);

        let tx = conn.transaction().unwrap();
        let mut graded_review = 0i64;
        let mut graded_learning = 0i64;
        let mut skipped_new = 0i64;
        let mut rng = Lcg(0x9E37_79B9);

        for (band, rate) in &pass_rates {
            let grade = grade_for(*rate);
            if grade == PreGrade::New {
                let n: i64 = tx
                    .query_row(
                        "SELECT COUNT(*) FROM words WHERE level = ?1 AND frequency_band = ?2",
                        rusqlite::params![PLACEMENT_LEVEL, band],
                        |r| r.get(0),
                    )
                    .unwrap();
                skipped_new += n;
                continue;
            }
            let ids: Vec<i64> = {
                let mut stmt = tx
                    .prepare(
                        "SELECT w.id FROM words w
                         WHERE w.level = ?1 AND w.frequency_band = ?2
                           AND NOT EXISTS (SELECT 1 FROM word_states s WHERE s.word_id = w.id)",
                    )
                    .unwrap();
                let rows = stmt
                    .query_map(rusqlite::params![PLACEMENT_LEVEL, band], |r| r.get(0))
                    .unwrap();
                rows.collect::<Result<_, _>>().unwrap()
            };
            let (lo, hi) = stability_range(*band);
            for id in ids {
                let stability = lo + rng.next_f64() * (hi - lo);
                word_states::upsert(
                    &tx,
                    &word_states::WordState {
                        word_id: id,
                        difficulty: 5.0,
                        stability,
                        due_at: clock::due_in_days(stability),
                        fsrs_state: if grade == PreGrade::Review { 2 } else { 1 },
                        app_state: grade.app_state().to_string(),
                        reps: 0,
                        lapses: 0,
                        question_level: grade.question_level(),
                        reinforce_streak: 0,
                        last_review_at: None,
                        mastered_at: None,
                    },
                )?;
                match grade {
                    PreGrade::Review => graded_review += 1,
                    PreGrade::Learning => graded_learning += 1,
                    PreGrade::New => {}
                }
            }
        }
        settings::set(&tx, "placement_stage", "2")?;
        crate::db::repo::player_stats::set_vocab_estimate(&tx, vocab_estimate)?;
        tx.commit().unwrap();

        Ok(PlacementOutcome {
            vocab_estimate,
            pass_rates: pass_rates.iter().map(|(_, r)| *r).collect(),
            graded_review,
            graded_learning,
            skipped_new,
        })
    }
}
