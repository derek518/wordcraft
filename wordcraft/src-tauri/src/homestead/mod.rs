//! 家园建造。spec §4.2 F9，plan: docs/plans/homestead-v1.1.md。
//!
//! 欢迎页已向用户承诺「收集的水晶可以用来建造家园」，在此兑现。

mod blueprints;
mod grants;

// grants 的函数只在本模块内使用，不对外导出——它们是发放规则的实现细节，
// command 层只暴露 grant_pending

use crate::db::{clock, repo::homestead as repo, repo::player_stats, Db};
use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct HomesteadState {
    pub grid: Vec<repo::PlacedBlock>,
    pub inventory: Vec<repo::BlockStock>,
    /// 网格边长。前端据此渲染，不硬编码
    pub grid_size: i64,
}

#[derive(Debug, Serialize)]
pub struct GrantOutcome {
    /// 本次新发放的 (类型, 数量)
    pub granted: Vec<(String, i64)>,
    pub total_available: i64,
}

fn lock(db: &Db) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
    db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))
}

fn snapshot(conn: &Connection) -> Result<HomesteadState, String> {
    Ok(HomesteadState {
        grid: repo::grid(conn)?,
        inventory: repo::inventory(conn)?,
        grid_size: repo::GRID_SIZE,
    })
}

/// 实际作答过的词 id。
///
/// `reps > 0` 而非 `word_states` 存在与否——摸底会为一千多个词预建状态，
/// 那是「估计你可能认识」，不是「你收集到了这颗水晶」。
fn answered_word_ids(conn: &Connection) -> Result<Vec<i64>, String> {
    let mut stmt = conn
        .prepare("SELECT word_id FROM word_states WHERE reps > 0 ORDER BY word_id")
        .map_err(|e| format!("准备已作答词查询失败: {e}"))?;
    let rows = stmt
        .query_map([], |r| r.get::<_, i64>(0))
        .map_err(|e| format!("查询已作答词失败: {e}"))?;
    rows.collect::<Result<_, _>>()
        .map_err(|e| format!("读取已作答词失败: {e}"))
}

/// 补发所有未发放的方块。启动时与会话结束后调用，幂等。
pub fn grant_pending(conn: &mut Connection) -> Result<GrantOutcome, String> {
    let answered = answered_word_ids(conn)?;
    let best_streak = player_stats::get(conn)?.best_streak;

    let pending: Vec<grants::PendingGrant> = [
        grants::mastery_grants(&answered, &repo::granted_keys(conn, "mastery")?),
        grants::streak_grants(best_streak, &repo::granted_keys(conn, "streak")?),
        grants::milestone_grants(
            answered.len() as i64,
            &repo::granted_keys(conn, "milestone")?,
        ),
    ]
    .concat();

    if pending.is_empty() {
        let inv = repo::inventory(conn)?;
        return Ok(GrantOutcome {
            granted: Vec::new(),
            total_available: inv.iter().map(|s| s.available).sum(),
        });
    }

    let now = clock::now();
    // 整批一个事务：中途失败会留下「账本记了但库存没加」的状态，
    // 而账本的唯一约束会让那些方块永远补不回来
    let tx = conn
        .transaction()
        .map_err(|e| format!("开启发放事务失败: {e}"))?;

    let mut tally: std::collections::HashMap<&str, i64> = std::collections::HashMap::new();
    for g in &pending {
        // record_grant 返回 false 表示已发过——并发调用下唯一约束会拦住，
        // 此时必须跳过加库存，否则同一块发两次
        if repo::record_grant(&tx, g.source, &g.source_key, g.block_type, g.amount, &now)? {
            repo::add_owned(&tx, g.block_type, g.amount)?;
            *tally.entry(g.block_type).or_insert(0) += g.amount;
        }
    }

    tx.commit().map_err(|e| format!("提交发放事务失败: {e}"))?;

    let inv = repo::inventory(conn)?;
    let mut granted: Vec<(String, i64)> =
        tally.into_iter().map(|(t, n)| (t.to_string(), n)).collect();
    granted.sort();

    if !granted.is_empty() {
        log::info!("发放方块: {granted:?}");
    }

    Ok(GrantOutcome {
        granted,
        total_available: inv.iter().map(|s| s.available).sum(),
    })
}

/// contracts §3.7：预置蓝图。静态数据，不落库——蓝图是产品内容，
/// 改动随版本发布，没有用户态可存。
#[tauri::command]
pub fn get_blueprints() -> Vec<blueprints::Blueprint> {
    blueprints::all()
}

/// 启动时补发。失败只记 warn 不阻断启动——方块是奖励，
/// 拿不到远不如打不开应用严重，下次启动会重试。
pub fn grant_on_startup(db: &Db) -> Result<(), String> {
    let mut conn = db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))?;
    let out = grant_pending(&mut conn)?;
    if !out.granted.is_empty() {
        log::info!("启动补发方块: {:?}", out.granted);
    }
    Ok(())
}

// ─────────────────────────────────────────────
// Commands
// ─────────────────────────────────────────────

#[tauri::command]
pub fn get_homestead(db: State<Db>) -> Result<HomesteadState, String> {
    let conn = lock(&db)?;
    snapshot(&conn)
}

/// 放置一块。返回整个快照而非 `()`——前端要同步更新网格与库存两处，
/// 让后端回一份权威状态，比前端各自推算再对账可靠。
#[tauri::command]
pub fn place_block(
    db: State<Db>,
    x: i64,
    y: i64,
    block_type: String,
) -> Result<HomesteadState, String> {
    let conn = lock(&db)?;
    repo::place(&conn, x, y, &block_type, &clock::now())?;
    snapshot(&conn)
}

#[tauri::command]
pub fn remove_block(db: State<Db>, x: i64, y: i64) -> Result<HomesteadState, String> {
    let conn = lock(&db)?;
    repo::remove(&conn, x, y)?;
    snapshot(&conn)
}

#[tauri::command]
pub fn grant_pending_blocks(db: State<Db>) -> Result<GrantOutcome, String> {
    let mut conn = db
        .0
        .lock()
        .map_err(|e| format!("获取数据库锁失败: {e}"))?;
    grant_pending(&mut conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{migrations, repo::word_states, repo::words};
    use crate::test_support::in_memory_db;

    fn db(answered: usize,預分级: usize) -> Connection {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();

        let total = answered + 預分级;
        let items: Vec<words::WordImport> = (0..total)
            .map(|i| {
                let w = format!("wd{}{}", (b'a' + (i / 26) as u8) as char,
                                (b'a' + (i % 26) as u8) as char);
                words::WordImport {
                    word: w.clone(),
                    phonetic: "/w/".into(),
                    pos: "n.".into(),
                    meaning: format!("释义{i}"),
                    example_1: format!("A {w} appears."),
                    example_2: String::new(),
                    level: "junior".into(),
                    frequency_band: 1,
                    zone: "newbie".into(),
                    source_edition: String::new(),
                }
            })
            .collect();
        let out = words::import(&mut conn, &items).unwrap();
        assert!(out.rejected.is_empty(), "测试数据被拒: {:?}", out.rejected);

        for i in 1..=total {
            // 前 answered 个是真答过的，其余模拟摸底预分级（reps = 0）
            let reps = if i <= answered { 3 } else { 0 };
            word_states::upsert(
                &conn,
                &word_states::WordState {
                    word_id: i as i64,
                    difficulty: 5.0,
                    stability: 10.0,
                    due_at: clock::now(),
                    fsrs_state: 2,
                    app_state: "review".into(),
                    reps,
                    lapses: 0,
                    question_level: 1,
                    reinforce_streak: 0,
                    last_review_at: None,
                    mastered_at: None,
                },
            )
            .unwrap();
        }
        conn
    }

    fn available(conn: &Connection, t: &str) -> i64 {
        repo::inventory(conn)
            .unwrap()
            .iter()
            .find(|s| s.block_type == t)
            .unwrap()
            .available
    }

    #[test]
    fn 只为实际作答的词发方块() {
        // 这是整个功能最关键的一条：照 spec 字面实现（有 word_states 就发），
        // 用户做完摸底立刻凭空得一千多块，建造第一天就失去意义
        let mut conn = db(10, 500);
        grant_pending(&mut conn).unwrap();

        assert_eq!(available(&conn, "normal"), 10, "摸底预分级的词不该产生方块");
    }

    #[test]
    fn 重复发放不增加库存() {
        let mut conn = db(10, 0);
        grant_pending(&mut conn).unwrap();
        let first = available(&conn, "normal");

        // 启动、会话结束都会触发，重复是常态
        for _ in 0..5 {
            grant_pending(&mut conn).unwrap();
        }
        assert_eq!(available(&conn, "normal"), first, "重复发放导致库存虚增");
    }

    #[test]
    fn 追溯后再答新词只增量发放() {
        // 11 个词：10 个已答，1 个摸底预分级
        let mut conn = db(10, 1);
        grant_pending(&mut conn).unwrap();
        assert_eq!(available(&conn, "normal"), 10, "预分级的那个不该计入");

        // 用户真的答了那个预分级的词——reps 从 0 变成 1
        conn.execute("UPDATE word_states SET reps = 1 WHERE word_id = 11", [])
            .unwrap();

        let out = grant_pending(&mut conn).unwrap();
        assert_eq!(available(&conn, "normal"), 11);
        assert_eq!(out.granted, vec![("normal".to_string(), 1)]);
    }

    #[test]
    fn 无待发放时返回空且不报错() {
        let mut conn = db(0, 100);
        let out = grant_pending(&mut conn).unwrap();
        assert!(out.granted.is_empty());
        assert_eq!(out.total_available, 0);
    }

    #[test]
    fn 里程碑随作答词数发放稀有方块() {
        let mut conn = db(250, 0);
        grant_pending(&mut conn).unwrap();
        // 250 词跨过 200 这一档
        assert_eq!(available(&conn, "rare"), 1);
    }

    #[test]
    fn 连续打卡发放限定方块() {
        let mut conn = db(5, 0);
        player_stats::set_streak(&conn, 15, "2026-08-08").unwrap();
        grant_pending(&mut conn).unwrap();

        // 15 天跨过 7 与 14 两档
        assert_eq!(available(&conn, "limited"), 2);
    }

    #[test]
    fn 发放后可放置且库存正确扣减() {
        let mut conn = db(3, 0);
        grant_pending(&mut conn).unwrap();

        repo::place(&conn, 0, 0, "normal", &clock::now()).unwrap();
        let state = snapshot(&conn).unwrap();

        assert_eq!(state.grid.len(), 1);
        assert_eq!(available(&conn, "normal"), 2);
        assert_eq!(state.grid_size, 20);
    }

    #[test]
    fn 快照包含全部三种类型即便数量为零() {
        let conn = db(0, 0);
        let state = snapshot(&conn).unwrap();
        // 前端要渲染三个槽位，缺类型会让界面少一格
        assert_eq!(state.inventory.len(), 3);
    }
}
