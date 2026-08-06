//! 作答结果持久化。契约见 contracts-v1.md §3.2。
//!
//! ADR-2：FSRS 计算在前端（`ts-fsrs`），Rust 侧只做校验与持久化。本模块因此
//! 不含任何间隔计算——它接收前端算好的 before/after 快照并原子落库。
//!
//! 「原子」是本模块存在的理由：`word_states` 与 `review_logs` 必须同生共死。
//! 若状态更新成功而日志失败，算法就失去了回溯调参的依据（spec §6）；若日志
//! 成功而状态失败，词的到期时间不变，会被立刻重新排入队列。

use crate::db::{
    clock,
    repo::{review_logs, sessions, word_states},
    Db,
};
use rusqlite::Connection;
use serde::Deserialize;
use tauri::State;

/// 合法的业务状态（contracts §4）。
const VALID_APP_STATES: [&str; 5] = ["new", "learning", "reinforcing", "review", "mastered"];

#[derive(Debug, Deserialize)]
pub struct FsrsBefore {
    pub difficulty: f64,
    pub stability: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsrsAfter {
    pub difficulty: f64,
    pub stability: f64,
    pub due_at: String,
    pub fsrs_state: i64,
    pub reps: i64,
    pub lapses: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewCommitDto {
    pub word_id: i64,
    pub session_id: Option<i64>,
    pub question_type: i64,
    pub is_correct: bool,
    pub reaction_ms: i64,
    pub rating: i64,
    pub before: FsrsBefore,
    pub after: FsrsAfter,
    pub app_state: String,
    pub question_level: i64,
    pub reinforce_streak: i64,
}

/// 边界校验。前端是不可信输入源——即便它是我们自己写的。
///
/// 数据库的 CHECK 约束是最后一道防线，但错误信息对定位问题无用；此处提前拦截
/// 并给出可诊断的消息。
fn validate(dto: &ReviewCommitDto) -> Result<(), String> {
    if !(1..=4).contains(&dto.rating) {
        return Err(format!("rating 必须在 1..4，收到 {}", dto.rating));
    }
    if !(1..=5).contains(&dto.question_type) {
        return Err(format!(
            "question_type 必须在 1..5，收到 {}",
            dto.question_type
        ));
    }
    if !(1..=5).contains(&dto.question_level) {
        return Err(format!(
            "question_level 必须在 1..5，收到 {}",
            dto.question_level
        ));
    }
    if !(0..=3).contains(&dto.after.fsrs_state) {
        return Err(format!(
            "fsrs_state 必须在 0..3，收到 {}",
            dto.after.fsrs_state
        ));
    }
    if !VALID_APP_STATES.contains(&dto.app_state.as_str()) {
        return Err(format!(
            "非法的 app_state `{}`，应为 {VALID_APP_STATES:?} 之一",
            dto.app_state
        ));
    }
    if dto.reaction_ms < 0 {
        return Err(format!("reaction_ms 不能为负，收到 {}", dto.reaction_ms));
    }
    if dto.reinforce_streak < 0 {
        return Err(format!(
            "reinforce_streak 不能为负，收到 {}",
            dto.reinforce_streak
        ));
    }
    if dto.after.stability < 0.0 || dto.after.difficulty < 0.0 {
        return Err(format!(
            "FSRS 状态不能为负: difficulty={}, stability={}",
            dto.after.difficulty, dto.after.stability
        ));
    }
    if dto.after.reps < 0 || dto.after.lapses < 0 {
        return Err("reps 与 lapses 不能为负".to_string());
    }
    // due_at 必须可解析——存进去再发现格式错误就晚了，排队查询会静默漏掉该词
    clock::parse_ts(&dto.after.due_at)
        .map_err(|e| format!("due_at `{}` 格式非法: {e}", dto.after.due_at))?;

    // 答对却给 Again、答错却给非 Again，说明前端评级逻辑与作答结果脱节
    if dto.is_correct && dto.rating == 1 {
        return Err("答对不应评为 Again(1)，前端评级与作答结果不一致".to_string());
    }
    if !dto.is_correct && dto.rating != 1 {
        return Err(format!(
            "答错必须评为 Again(1)，收到 {}，前端评级与作答结果不一致",
            dto.rating
        ));
    }

    Ok(())
}

/// 推导 `mastered_at`。
///
/// 时间戳由后端产生而非前端传入——前端已经负责状态机判定，不必再关心时间。
fn resolve_mastered_at(
    previous: Option<&word_states::WordState>,
    next_app_state: &str,
    now: &str,
) -> Option<String> {
    if next_app_state != "mastered" {
        return None;
    }
    // 已是 mastered 则保留原始达成时间，不因每次抽查而刷新
    match previous {
        Some(p) if p.app_state == "mastered" => p.mastered_at.clone().or_else(|| Some(now.into())),
        _ => Some(now.into()),
    }
}

/// 在单一事务中落库作答结果。
pub fn commit(conn: &mut Connection, dto: &ReviewCommitDto) -> Result<(), String> {
    validate(dto)?;

    let now = clock::now();
    let previous = word_states::get(conn, dto.word_id)?;
    let mastered_at = resolve_mastered_at(previous.as_ref(), &dto.app_state, &now);

    let tx = conn
        .transaction()
        .map_err(|e| format!("开启作答提交事务失败: {e}"))?;

    review_logs::insert(
        &tx,
        &review_logs::NewReviewLog {
            word_id: dto.word_id,
            session_id: dto.session_id,
            question_type: dto.question_type,
            is_correct: dto.is_correct,
            reaction_ms: dto.reaction_ms,
            rating: dto.rating,
            difficulty_before: dto.before.difficulty,
            stability_before: dto.before.stability,
            difficulty_after: dto.after.difficulty,
            stability_after: dto.after.stability,
        },
        &now,
    )?;

    word_states::upsert(
        &tx,
        &word_states::WordState {
            word_id: dto.word_id,
            difficulty: dto.after.difficulty,
            stability: dto.after.stability,
            due_at: dto.after.due_at.clone(),
            fsrs_state: dto.after.fsrs_state,
            app_state: dto.app_state.clone(),
            reps: dto.after.reps,
            lapses: dto.after.lapses,
            question_level: dto.question_level,
            reinforce_streak: dto.reinforce_streak,
            last_review_at: Some(now.clone()),
            mastered_at,
        },
    )?;

    // 会话进度随每题递增，而非等到会话结束才写。
    // 决议 S13 后单场约 4 分钟，中途退出是常态——已作答的部分必须留存。
    if let Some(session_id) = dto.session_id {
        sessions::record_answer(&tx, session_id)?;
    }

    tx.commit()
        .map_err(|e| format!("提交作答事务失败: {e}"))
}

/// contracts §3.2
#[tauri::command]
pub fn commit_review(db: State<Db>, payload: ReviewCommitDto) -> Result<(), String> {
    let mut conn = db
        .0
        .lock()
        .map_err(|e| format!("获取数据库锁失败: {e}"))?;
    commit(&mut conn, &payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;
    use crate::db::repo::words;
    use crate::test_support::in_memory_db;

    fn setup() -> Connection {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();
        let out = words::import(
            &mut conn,
            &[words::WordImport {
                word: "crystal".into(),
                phonetic: "/ˈkrɪstl/".into(),
                pos: "n.".into(),
                meaning: "水晶".into(),
                example_1: "A glowing crystal lights the cave.".into(),
                example_2: String::new(),
                level: "junior".into(),
                frequency_band: 1,
                zone: "newbie".into(),
                source_edition: String::new(),
            }],
        )
        .unwrap();
        assert!(out.rejected.is_empty());
        conn
    }

    fn dto() -> ReviewCommitDto {
        ReviewCommitDto {
            word_id: 1,
            session_id: None,
            question_type: 1,
            is_correct: true,
            reaction_ms: 2500,
            rating: 4,
            before: FsrsBefore {
                difficulty: 5.0,
                stability: 1.0,
            },
            after: FsrsAfter {
                difficulty: 4.8,
                stability: 3.2,
                due_at: clock::due_in_days(3.2),
                fsrs_state: 2,
                reps: 1,
                lapses: 0,
            },
            app_state: "review".into(),
            question_level: 2,
            reinforce_streak: 0,
        }
    }

    // ── 正常路径 ──────────────────────────

    #[test]
    fn 一次提交同时写入状态与日志() {
        let mut conn = setup();
        commit(&mut conn, &dto()).unwrap();

        let state = word_states::get(&conn, 1).unwrap().expect("状态未写入");
        assert_eq!(state.app_state, "review");
        assert_eq!(state.reps, 1);
        assert!((state.stability - 3.2).abs() < 1e-9);
        assert!(state.last_review_at.is_some(), "last_review_at 未由后端填充");

        assert_eq!(review_logs::total_count(&conn).unwrap(), 1, "日志未写入");
    }

    #[test]
    fn 日志保留前后快照供算法回溯() {
        let mut conn = setup();
        commit(&mut conn, &dto()).unwrap();

        let (d_before, s_before, d_after, s_after): (f64, f64, f64, f64) = conn
            .query_row(
                "SELECT difficulty_before, stability_before, difficulty_after, stability_after
                 FROM review_logs WHERE word_id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();

        assert!((d_before - 5.0).abs() < 1e-9);
        assert!((s_before - 1.0).abs() < 1e-9);
        assert!((d_after - 4.8).abs() < 1e-9);
        assert!((s_after - 3.2).abs() < 1e-9);
    }

    #[test]
    fn 重复提交累积日志但状态只保留最新() {
        let mut conn = setup();
        commit(&mut conn, &dto()).unwrap();

        let mut second = dto();
        second.after.reps = 2;
        second.after.stability = 8.0;
        commit(&mut conn, &second).unwrap();

        assert_eq!(review_logs::total_count(&conn).unwrap(), 2);
        let state = word_states::get(&conn, 1).unwrap().unwrap();
        assert_eq!(state.reps, 2);
        assert!((state.stability - 8.0).abs() < 1e-9);
    }

    // ── 事务原子性 ──────────────────────────

    #[test]
    fn 会话不存在时状态与日志都不写入() {
        let mut conn = setup();
        let mut d = dto();
        d.session_id = Some(999); // 外键指向不存在的会话

        let result = commit(&mut conn, &d);
        assert!(result.is_err(), "非法 session_id 应被拒绝");

        assert!(
            word_states::get(&conn, 1).unwrap().is_none(),
            "事务失败后状态不应残留"
        );
        assert_eq!(
            review_logs::total_count(&conn).unwrap(),
            0,
            "事务失败后日志不应残留"
        );
    }

    #[test]
    fn 词条不存在时整笔提交回滚() {
        let mut conn = setup();
        let mut d = dto();
        d.word_id = 404;

        assert!(commit(&mut conn, &d).is_err());
        assert_eq!(review_logs::total_count(&conn).unwrap(), 0);
    }

    #[test]
    fn 会话计数随每题递增() {
        let mut conn = setup();
        let today = clock::today();
        let s = sessions::start(&conn, &today, "morning", 20, &clock::now()).unwrap();

        let mut d = dto();
        d.session_id = Some(s.id);
        commit(&mut conn, &d).unwrap();

        let after = sessions::find_by_id(&conn, s.id).unwrap().unwrap();
        assert_eq!(
            after.completed_count, 1,
            "会话进度未随作答递增——中途退出会丢失已完成部分"
        );
    }

    // ── 校验 ──────────────────────────

    #[test]
    fn 越界的评级与题型被拒绝() {
        let mut conn = setup();

        for bad_rating in [0, 5, -1] {
            let mut d = dto();
            d.rating = bad_rating;
            assert!(
                commit(&mut conn, &d).is_err(),
                "rating={bad_rating} 应被拒绝"
            );
        }
        for bad_qt in [0, 6] {
            let mut d = dto();
            d.question_type = bad_qt;
            assert!(commit(&mut conn, &d).is_err(), "question_type={bad_qt} 应被拒绝");
        }
        assert_eq!(review_logs::total_count(&conn).unwrap(), 0);
    }

    #[test]
    fn 非法状态值被拒绝且消息可诊断() {
        let mut conn = setup();
        let mut d = dto();
        d.app_state = "mastered_maybe".into();

        let err = commit(&mut conn, &d).unwrap_err();
        assert!(
            err.contains("mastered_maybe") && err.contains("app_state"),
            "错误消息缺少诊断信息: {err}"
        );
    }

    #[test]
    fn 无法解析的到期时间被拒绝() {
        let mut conn = setup();
        let mut d = dto();
        d.after.due_at = "2026年8月6日".into();

        let err = commit(&mut conn, &d).unwrap_err();
        assert!(err.contains("due_at"), "错误消息未指明是 due_at: {err}");
        // 若放过，该词的 due_at 无法参与比较，会从排队查询中静默消失
        assert!(word_states::get(&conn, 1).unwrap().is_none());
    }

    #[test]
    fn 评级与作答结果矛盾时被拒绝() {
        let mut conn = setup();

        let mut wrong_but_good = dto();
        wrong_but_good.is_correct = false;
        wrong_but_good.rating = 3;
        assert!(
            commit(&mut conn, &wrong_but_good).is_err(),
            "答错却评 Good 应被拒绝"
        );

        let mut right_but_again = dto();
        right_but_again.is_correct = true;
        right_but_again.rating = 1;
        assert!(
            commit(&mut conn, &right_but_again).is_err(),
            "答对却评 Again 应被拒绝"
        );
    }

    #[test]
    fn 负数字段被拒绝() {
        let mut conn = setup();

        let mut neg_reaction = dto();
        neg_reaction.reaction_ms = -1;
        assert!(commit(&mut conn, &neg_reaction).is_err());

        let mut neg_stability = dto();
        neg_stability.after.stability = -0.1;
        assert!(commit(&mut conn, &neg_stability).is_err());
    }

    // ── mastered_at 推导 ──────────────────────────

    #[test]
    fn 首次进入已掌握时记录达成时间() {
        let mut conn = setup();
        let mut d = dto();
        d.app_state = "mastered".into();
        d.question_level = 4;
        commit(&mut conn, &d).unwrap();

        let state = word_states::get(&conn, 1).unwrap().unwrap();
        assert!(state.mastered_at.is_some(), "未记录 mastered_at");
    }

    #[test]
    fn 已掌握词再次抽查不刷新达成时间() {
        let mut conn = setup();
        let mut d = dto();
        d.app_state = "mastered".into();
        d.question_level = 4;
        commit(&mut conn, &d).unwrap();
        let first = word_states::get(&conn, 1).unwrap().unwrap().mastered_at;

        let mut again = dto();
        again.app_state = "mastered".into();
        again.question_level = 4;
        again.after.reps = 2;
        commit(&mut conn, &again).unwrap();

        let second = word_states::get(&conn, 1).unwrap().unwrap().mastered_at;
        assert_eq!(first, second, "达成时间不应被后续抽查覆盖");
    }

    #[test]
    fn 掌握度回落时清空达成时间() {
        let mut conn = setup();
        let mut d = dto();
        d.app_state = "mastered".into();
        d.question_level = 4;
        commit(&mut conn, &d).unwrap();
        assert!(word_states::get(&conn, 1).unwrap().unwrap().mastered_at.is_some());

        // 抽查失败 → 回落强化队列
        let mut lapsed = dto();
        lapsed.is_correct = false;
        lapsed.rating = 1;
        lapsed.app_state = "reinforcing".into();
        lapsed.after.lapses = 1;
        commit(&mut conn, &lapsed).unwrap();

        let state = word_states::get(&conn, 1).unwrap().unwrap();
        assert_eq!(state.app_state, "reinforcing");
        assert!(state.mastered_at.is_none(), "回落后应清空 mastered_at");
    }
}
