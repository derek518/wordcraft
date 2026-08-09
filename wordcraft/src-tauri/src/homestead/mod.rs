//! 家园建造。spec §4.2 F9，plan: docs/plans/homestead-v1.1.md。
//!
//! 欢迎页已向用户承诺「收集的水晶可以用来建造家园」，在此兑现。

mod blueprints;
mod grants;
mod residents;

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

// ─────────────────────────────────────────────
// 居民
// ─────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ResidentsState {
    /// 已解锁的入住位
    pub slots: i64,
    pub max_slots: i64,
    /// 已建成的蓝图 id，按阶段顺序
    pub completed: Vec<String>,
    pub residents: Vec<repo::Resident>,
    /// 已收集但未入住的生物
    pub candidates: Vec<repo::Resident>,
    pub digest: Digest,
}

/// 居民转述的真实数据。
///
/// 家园此前是只出不进的水槽——建完就没事做了。让住户报几个当下的数字，
/// 它就同时是个软性的信息面板，而不用另做一套通知。
/// 数字在后端算，措辞留给前端：事实只能有一个来源，说法可以有很多种。
#[derive(Debug, Serialize)]
pub struct Digest {
    pub due_count: i64,
    pub available_blocks: i64,
    pub streak: i64,
    /// 距下一个词量里程碑还差多少词；已全部达成时为 0
    pub words_to_milestone: i64,
}

fn digest(conn: &Connection) -> Result<Digest, String> {
    let now = clock::now();
    let due_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM word_states WHERE reps > 0 AND due_at <= ?1",
            [&now],
            |r| r.get(0),
        )
        .map_err(|e| format!("统计到期词失败: {e}"))?;

    let answered = answered_word_ids(conn)?.len() as i64;
    let words_to_milestone = grants::words_to_next_milestone(answered);

    let stats = player_stats::get(conn)?;
    let available_blocks = repo::inventory(conn)?.iter().map(|s| s.available).sum();

    Ok(Digest {
        due_count,
        available_blocks,
        streak: stats.current_streak,
        words_to_milestone,
    })
}

fn residents_snapshot(conn: &Connection) -> Result<ResidentsState, String> {
    let done = residents::completed(&repo::grid(conn)?, &blueprints::all());
    let slots = residents::slots_for(done.len());

    // 拆掉一块方块，蓝图就不再成立，位置随之收回。住在里面的居民
    // 必须同时搬走，否则会留下一条前端读不出来的记录
    let evicted = repo::evict_beyond(conn, slots)?;
    if evicted > 0 {
        log::info!("蓝图不再成立，{evicted} 位居民搬离");
    }

    Ok(ResidentsState {
        slots,
        max_slots: residents::max_slots(),
        completed: done,
        residents: repo::residents(conn)?,
        candidates: repo::resident_candidates(conn)?,
        digest: digest(conn)?,
    })
}

#[tauri::command]
pub fn get_residents(db: State<Db>) -> Result<ResidentsState, String> {
    let conn = lock(&db)?;
    residents_snapshot(&conn)
}

#[tauri::command]
pub fn move_in_resident(db: State<Db>, slot: i64, card_id: i64) -> Result<ResidentsState, String> {
    let conn = lock(&db)?;

    // 位置上限由已建成的蓝图决定。前端拿到的槽位数可能已经过期
    // （比如另一个界面刚拆了方块），以此刻的实际状态为准
    let done = residents::completed(&repo::grid(&conn)?, &blueprints::all());
    let slots = residents::slots_for(done.len());
    if !(0..slots).contains(&slot) {
        return Err(format!("位置 {slot} 尚未解锁，当前只有 {slots} 个"));
    }

    repo::move_in(&conn, slot, card_id, &clock::now())?;
    residents_snapshot(&conn)
}

#[tauri::command]
pub fn move_out_resident(db: State<Db>, slot: i64) -> Result<ResidentsState, String> {
    let conn = lock(&db)?;
    repo::move_out(&conn, slot)?;
    residents_snapshot(&conn)
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

    /// 居民相关的 SQL 只有跑起来才会暴露列名与联表错误。
    mod 居民 {
        use super::*;

        /// 建好小屋（24 块普通方块），并收集若干张卡。
        fn 有小屋的家园(collect: &[i64]) -> Connection {
            let mut conn = db(30, 0);
            grant_pending(&mut conn).unwrap();

            let hut = blueprints::all().into_iter().next().unwrap();
            for c in &hut.cells {
                repo::place(&conn, c.x, c.y, &c.block_type, &clock::now()).unwrap();
            }

            for id in collect {
                conn.execute(
                    "INSERT INTO card_collection (card_id, count, first_at, is_new)
                     VALUES (?1, 1, ?2, 0)",
                    rusqlite::params![id, clock::now()],
                )
                .unwrap();
            }
            conn
        }

        /// 卡池里第一张活物卡，与第一张非活物卡（碎片/器物/神器）的 id。
        fn 卡池(conn: &Connection) -> (i64, i64) {
            let living = conn
                .query_row(
                    "SELECT id FROM cards WHERE card_type IN ('creature','guardian') ORDER BY id LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            let thing = conn
                .query_row(
                    "SELECT id FROM cards WHERE card_type NOT IN ('creature','guardian') ORDER BY id LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            (living, thing)
        }

        #[test]
        fn 建成小屋解锁第一个入住位() {
            let conn = 有小屋的家园(&[]);
            let s = residents_snapshot(&conn).unwrap();
            assert_eq!(s.completed, vec!["hut"]);
            assert_eq!(s.slots, 1, "建成一张蓝图应解锁一个位置");
        }

        #[test]
        fn 未建成任何蓝图时没有入住位() {
            let conn = db(30, 0);
            let s = residents_snapshot(&conn).unwrap();
            assert_eq!(s.slots, 0);
            assert!(s.residents.is_empty());
        }

        #[test]
        fn 只有已收集的生物出现在候选里() {
            let mut conn = db(30, 0);
            migrations::run(&mut conn).unwrap();
            let (creature, painting) = 卡池(&conn);
            let conn = 有小屋的家园(&[creature, painting]);

            let s = residents_snapshot(&conn).unwrap();
            let ids: Vec<i64> = s.candidates.iter().map(|c| c.card_id).collect();
            assert_eq!(ids, vec![creature], "候选应只含已收集的活物");
        }

        #[test]
        fn 入住后从候选里消失() {
            let mut conn = db(30, 0);
            migrations::run(&mut conn).unwrap();
            let (creature, _) = 卡池(&conn);
            let conn = 有小屋的家园(&[creature]);

            repo::move_in(&conn, 0, creature, &clock::now()).unwrap();
            let s = residents_snapshot(&conn).unwrap();

            assert_eq!(s.residents.len(), 1);
            assert_eq!(s.residents[0].card_id, creature);
            assert!(!s.residents[0].name.is_empty(), "应联表取到卡牌名");
            assert!(s.candidates.is_empty(), "已入住的不该还在候选里");
        }

        #[test]
        fn 未收集的卡不能入住() {
            let mut conn = db(30, 0);
            migrations::run(&mut conn).unwrap();
            let (creature, _) = 卡池(&conn);
            let conn = 有小屋的家园(&[]); // 一张都没收集

            let err = repo::move_in(&conn, 0, creature, &clock::now()).unwrap_err();
            assert!(err.contains("尚未收集"), "{err}");
        }

        #[test]
        fn 碎片器物不能入住() {
            let mut conn = db(30, 0);
            migrations::run(&mut conn).unwrap();
            let (_, painting) = 卡池(&conn);
            let conn = 有小屋的家园(&[painting]);

            // 碎片、器物、神器是东西，不是住户
            let err = repo::move_in(&conn, 0, painting, &clock::now()).unwrap_err();
            assert!(err.contains("活物"), "{err}");
        }

        #[test]
        fn 同一只生物不能占两个位置() {
            let mut conn = db(30, 0);
            migrations::run(&mut conn).unwrap();
            let (creature, _) = 卡池(&conn);
            let conn = 有小屋的家园(&[creature]);

            repo::move_in(&conn, 0, creature, &clock::now()).unwrap();
            // 唯一约束挡下分身。否则一张稀有卡就能填满所有位置
            assert!(repo::move_in(&conn, 1, creature, &clock::now()).is_err());
        }

        #[test]
        fn 拆掉方块使蓝图失效时居民自动搬离() {
            let mut conn = db(30, 0);
            migrations::run(&mut conn).unwrap();
            let (creature, _) = 卡池(&conn);
            let conn = 有小屋的家园(&[creature]);

            repo::move_in(&conn, 0, creature, &clock::now()).unwrap();
            assert_eq!(residents_snapshot(&conn).unwrap().residents.len(), 1);

            // 拆掉小屋的一块，蓝图不再成立
            let hut = blueprints::all().into_iter().next().unwrap();
            let c = &hut.cells[0];
            repo::remove(&conn, c.x, c.y).unwrap();

            let s = residents_snapshot(&conn).unwrap();
            assert_eq!(s.slots, 0);
            assert!(s.residents.is_empty(), "位置收回后居民必须搬走");
            // 搬走的生物要能重新入住，不能凭空消失
            assert_eq!(s.candidates.len(), 1);
        }

        #[test]
        fn 简报数字取自真实状态() {
            let mut conn = db(30, 0);
            grant_pending(&mut conn).unwrap();
            let d = digest(&conn).unwrap();

            assert_eq!(d.available_blocks, 30, "30 个作答词应发 30 块");
            // 里程碑首档 200 词，已答 30
            assert_eq!(d.words_to_milestone, 170);
        }

    }
}
