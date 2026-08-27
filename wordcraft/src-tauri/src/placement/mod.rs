//! 摸底：给能力估计一个起点。contracts §9。
//!
//! ## 从「逐词预分级」改成「只定 θ」
//!
//! 原设计考 60 道初中词，按频段整段判定：band 1 对 11 题就把那 1067 个词
//! 全部标成「已掌握」，对 9 题就整段降级。60 题产出 5 个桶的结论，覆盖
//! 1600 个词——而且只覆盖初中词，senior 与 cet4 一律当新词。
//!
//! 它还预建了约 1438 条 `word_states`，那些词因此被挡在新词队列之外，
//! 依据只是一次频段级的猜测。
//!
//! 现在这件事由能力模型做，而且更细：θ 给出**每个词**的掌握概率，并且
//! 每天的作答都在修正它（见 `ability.rs` 与契约 §13）。摸底只剩一个职责——
//! **给 θ 一个起点**，免得头几场把难度放错。
//!
//! ## 为什么只有 20 题
//!
//! 一次性摸底不可能精确：四选一有 25% 的猜对下限，信息量存在上限。模拟显示
//! 20 题已能把「首场难度放对」的比例从 0%（纯先验，水平偏离时必错）提到
//! 九成，再加题收益很小。剩下的精度靠日常作答积累——一周的观测量远超任何
//! 摸底测试。
//!
//! ## 不再写 word_states
//!
//! 摸底答对一次不等于掌握。真要跳过某个词，让 θ 去判——它对每个词都有概率，
//! 而且会随作答修正。预建状态是把一次性的猜测**固化**成不可见的过滤条件。

use crate::ability;
use crate::commands::stats::AbilityOverview;
use crate::db::{clock, repo::player_stats, repo::settings, Db};
use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

/// 摸底题数。
///
/// 由模拟选定，见 `ability::PLACEMENT_PRIOR_INFORMATION` 的注释表。
pub const ITEMS: i64 = 20;

#[derive(Debug, Serialize)]
pub struct PlacementQuestion {
    pub word_id: i64,
    pub word: String,
    pub phonetic: String,
    pub pos: String,
    pub meaning: String,
    /// 该词的词频排名。界面用它显示「这题有多难」
    pub frequency_rank: i64,
    pub answered: i64,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct AnswerOutcome {
    pub answered: i64,
    pub total: i64,
    /// 题目答完了，可以调 finalize
    pub placement_done: bool,
}

fn lock(db: &Db) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
    db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))
}

fn answered_count(conn: &Connection) -> Result<i64, String> {
    conn.query_row("SELECT COUNT(*) FROM placement_asked", [], |r| r.get(0))
        .map_err(|e| format!("统计摸底已出题数失败: {e}"))
}

/// 取离当前能力边界最近、且没问过的词。
///
/// 信息量最大的就是这一点：那里答对答错各半，每一题都真正改变估计。
/// 问第 1 名的词答对说明不了任何事，问第 40000 名答对多半是蒙的。
///
/// **不受学习范围约束**：范围是「想练哪本考纲」，而这里在测能力，
/// 用全库才测得准。
fn pick_question(conn: &Connection, theta: f64) -> Result<Option<PlacementQuestion>, String> {
    let boundary = ability::vocabulary_rank(theta);
    let answered = answered_count(conn)?;

    let dist = ability::distance_sql("w.frequency_rank", boundary);
    conn.query_row(
        &format!(
        "SELECT w.id, w.word, w.phonetic, w.pos, w.meaning, w.frequency_rank
           FROM words w
          WHERE w.frequency_rank IS NOT NULL
            AND NOT EXISTS (SELECT 1 FROM placement_asked a WHERE a.word_id = w.id)
          ORDER BY {dist}
          LIMIT 1"
        ),
        [],
        |r| {
            Ok(PlacementQuestion {
                word_id: r.get(0)?,
                word: r.get(1)?,
                phonetic: r.get(2)?,
                pos: r.get(3)?,
                meaning: r.get(4)?,
                frequency_rank: r.get(5)?,
                answered,
                total: ITEMS,
            })
        },
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(format!("挑选摸底题失败: {other}")),
    })
}

// ─────────────────────────────────────────────
// Commands
// ─────────────────────────────────────────────

/// contracts §3.6：取下一道摸底题。返回 None 表示摸底已结束。
#[tauri::command]
pub fn get_placement_question(db: State<Db>) -> Result<Option<PlacementQuestion>, String> {
    let conn = lock(&db)?;
    next_question(&conn)
}

/// 与 command 分开，测试才打得到**真代码**上。
pub fn next_question(conn: &Connection) -> Result<Option<PlacementQuestion>, String> {
    settings::set(conn, "placement_stage", "1")?;

    let answered = answered_count(conn)?;
    if answered >= ITEMS {
        return Ok(None);
    }

    // 第一题之前把能力重置到摸底起点：先验弱得多，好让真实作答快速带走估计
    // （见 ability::PLACEMENT_PRIOR_INFORMATION）
    if answered == 0 {
        player_stats::set_ability(conn, &ability::Ability::for_placement())?;
    }

    let theta = player_stats::ability(conn)?.theta;
    pick_question(conn, theta)
}

/// 提交一题的作答。
///
/// 与日常作答喂给 θ 的是**同一种观测**——都是没教过就直接考。区别只是摸底
/// 把它们集中在两分钟里问完。
#[tauri::command]
pub fn submit_placement_answer(
    db: State<Db>,
    word_id: i64,
    is_correct: bool,
    reaction_ms: i64,
) -> Result<AnswerOutcome, String> {
    let mut conn = lock(&db)?;
    record_answer(&mut conn, word_id, is_correct, reaction_ms)
}

/// 与 command 分开，测试才打得到**真代码**上。
/// 在测试里自己重写一遍更新逻辑的话，改坏生产代码它一声不吭。
pub fn record_answer(
    conn: &mut Connection,
    word_id: i64,
    is_correct: bool,
    reaction_ms: i64,
) -> Result<AnswerOutcome, String> {
    if reaction_ms < 0 {
        return Err(format!("reaction_ms 不能为负，收到 {reaction_ms}"));
    }

    let rank: Option<i64> = conn
        .query_row(
            "SELECT frequency_rank FROM words WHERE id = ?1",
            [word_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("读取词频排名失败: {e}"))?;
    let Some(rank) = rank else {
        return Err(format!("词 {word_id} 没有词频排名，不该作为摸底题出现"));
    };

    let tx = conn
        .transaction()
        .map_err(|e| format!("开启摸底事务失败: {e}"))?;

    tx.execute(
        "INSERT INTO placement_asked (word_id, asked_at) VALUES (?1, ?2)
         ON CONFLICT(word_id) DO NOTHING",
        rusqlite::params![word_id, clock::now()],
    )
    .map_err(|e| format!("记录已出题失败: {e}"))?;

    let prior = player_stats::ability(&tx)?;
    let next = ability::update(prior, rank, is_correct);
    player_stats::set_ability(&tx, &next)?;
    let vocab = player_stats::vocab_from_ability(&tx, &next)?;
    player_stats::set_vocab_estimate(&tx, vocab)?;

    let answered = answered_count(&tx)?;
    tx.commit().map_err(|e| format!("提交摸底事务失败: {e}"))?;

    Ok(AnswerOutcome {
        answered,
        total: ITEMS,
        placement_done: answered >= ITEMS,
    })
}

/// contracts §3.6：结束摸底，返回能力概览。
///
/// 返回的就是设置页那张卡的内容——摸底不再产出一套自己的数字，
/// 它和日常作答更新的是同一个估计。
#[tauri::command]
pub fn finalize_placement(db: State<Db>) -> Result<AbilityOverview, String> {
    let conn = lock(&db)?;
    settings::set(&conn, "placement_stage", "2")?;
    let a = player_stats::ability(&conn)?;
    crate::commands::stats::overview_of(&conn, &a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;
    use crate::db::repo::words;
    use crate::test_support::in_memory_db;

    /// 造一个词频跨度覆盖整个量程的词库。
    fn db() -> Connection {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();
        let items: Vec<words::WordImport> = (0..60)
            .map(|i| {
                let w = format!("pw{}{}", (b'a' + (i / 26) as u8) as char, (b'a' + (i % 26) as u8) as char);
                words::WordImport {
                    example_1: format!("A {w} appears."),
                    word: w,
                    phonetic: "/w/".into(),
                    pos: "n.".into(),
                    meaning: format!("释义{i}"),
                    example_2: String::new(),
                    level: "senior".into(),
                    frequency_band: 1,
                    // 第 1 名到第 32768 名，等比铺开
                    frequency_rank: Some(1 << (i % 16)),
                    zone: "newbie".into(),
                    source_edition: String::new(),
                }
            })
            .collect();
        assert!(words::import(&mut conn, &items).unwrap().rejected.is_empty());
        conn
    }

    /// 走真正的 `next_question`，绕开的只是 tauri 的 State
    fn ask(conn: &Connection) -> Option<PlacementQuestion> {
        next_question(conn).unwrap()
    }

    /// 走真正的 `record_answer`，不在测试里重写一遍更新逻辑
    fn answer(conn: &mut Connection, q: &PlacementQuestion, correct: bool) {
        record_answer(conn, q.word_id, correct, 1200).unwrap();
    }

    #[test]
    fn 取离能力边界最近的词() {
        let conn = db();
        let boundary = ability::vocabulary_rank(ability::PRIOR_THETA);

        let q = ask(&conn).expect("应有题");
        // 词库里离 boundary 最近的那个 2 的幂
        let best = (0..16)
            .map(|i| 1i64 << i)
            .min_by_key(|r| (r - boundary).abs())
            .unwrap();
        // 问第 1 名的词答对说明不了任何事，问第 32768 名答对多半是蒙的。
        // 信息量最大的是边界附近
        assert_eq!(q.frequency_rank, best, "应取离边界最近的难度");
    }

    #[test]
    fn 同一个词不会问两次() {
        let mut conn = db();

        let mut seen = std::collections::HashSet::new();
        for _ in 0..12 {
            let q = ask(&conn).expect("应有题");
            assert!(seen.insert(q.word_id), "词 {} 被重复出题", q.word);
            answer(&mut conn, &q, true);
        }
    }

    #[test]
    fn 答对提升估计答错降低估计() {
        let mut conn = db();
        let before = player_stats::ability(&conn).unwrap().theta;

        let q = ask(&conn).expect("应有题");
        answer(&mut conn, &q, true);
        let after_right = player_stats::ability(&conn).unwrap().theta;
        assert!(after_right > before);

        let q = ask(&conn).expect("应有题");
        answer(&mut conn, &q, false);
        assert!(player_stats::ability(&conn).unwrap().theta < after_right);
    }

    #[test]
    fn 一路答对会走到高难度题() {
        let mut conn = db();
        let first = ask(&conn).unwrap().frequency_rank;

        for _ in 0..10 {
            let q = ask(&conn).expect("应有题");
            answer(&mut conn, &q, true);
        }
        let now = ask(&conn).unwrap().frequency_rank;

        // 楼梯法的核心：答对就往难处走。不走的话 20 题全在同一个难度上，
        // 等于问了 20 遍同一个问题
        assert!(now > first, "连续答对后应出更难的题（{first} → {now}）");
    }

    #[test]
    fn 一路答错会走到低难度题() {
        let mut conn = db();
        let first = ask(&conn).unwrap().frequency_rank;

        for _ in 0..10 {
            let q = ask(&conn).expect("应有题");
            answer(&mut conn, &q, false);
        }
        assert!(ask(&conn).unwrap().frequency_rank < first, "连续答错后应出更简单的题");
    }

    #[test]
    fn 摸底不写入任何词状态() {
        let mut conn = db();
        for _ in 0..10 {
            let q = ask(&conn).expect("应有题");
            answer(&mut conn, &q, true);
        }

        // 原设计预建约 1438 条 word_states，那些词因此被挡在新词队列之外，
        // 依据只是一次频段级的猜测。摸底答对一次不等于掌握——
        // 真要跳过某个词，让 θ 去判
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM word_states", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0, "摸底不该预建词状态");
    }

    #[test]
    fn 拒绝负的反应时间() {
        let mut conn = db();
        assert!(record_answer(&mut conn, 1, true, -1).is_err());
    }

    #[test]
    fn 无排名的词提交时报错而不是静默跳过() {
        let mut conn = db();
        conn.execute("UPDATE words SET frequency_rank = NULL WHERE id = 1", []).unwrap();
        // 挑题时已经排除了无排名的词，走到这里说明有别的路径把它塞了进来。
        // 静默跳过会让摸底少一次观测且无人知晓
        assert!(record_answer(&mut conn, 1, true, 900).is_err());
    }

    #[test]
    fn 提交后返回的已答数与总题数正确() {
        let mut conn = db();
        let q = ask(&conn).unwrap();
        let out = record_answer(&mut conn, q.word_id, true, 900).unwrap();
        assert_eq!((out.answered, out.total, out.placement_done), (1, ITEMS, false));
    }

    #[test]
    fn 答满题数后标记完成() {
        let mut conn = db();
        let mut last = None;
        for _ in 0..ITEMS {
            let q = ask(&conn).expect("应有题");
            last = Some(record_answer(&mut conn, q.word_id, true, 900).unwrap());
        }
        assert!(last.unwrap().placement_done, "答满 {ITEMS} 题应标记完成");
    }

    #[test]
    fn 无词频排名的词不会被出题() {
        let conn = db();
        conn.execute("UPDATE words SET frequency_rank = NULL", []).unwrap();
        // 难度未知的词问了也不知道说明什么
        assert!(ask(&conn).is_none(), "全库无排名时不该出题");
    }

    #[test]
    fn 首题之前把能力重置到摸底起点() {
        let conn = db();
        // 先塞一个和摸底起点完全不同的估计
        player_stats::set_ability(
            &conn,
            &ability::Ability { theta: 14.0, information: 9.0, observations: 99 },
        )
        .unwrap();

        ask(&conn).expect("应有题");

        let a = player_stats::ability(&conn).unwrap();
        // 不重置的话，摸底会从一个陈旧的强估计出发——20 题根本推不动它，
        // 等于白测
        assert_eq!(a.theta, ability::PRIOR_THETA);
        assert_eq!(a.information, ability::PLACEMENT_PRIOR_INFORMATION);
        assert_eq!(a.observations, 0);
    }

    #[test]
    fn 中途不再重置已经积累的估计() {
        let mut conn = db();
        let q = ask(&conn).expect("应有题");
        record_answer(&mut conn, q.word_id, true, 900).unwrap();
        let after_first = player_stats::ability(&conn).unwrap();

        ask(&conn).expect("应有题");

        // 每取一题就重置的话，摸底永远停在第一题的水平
        assert_eq!(player_stats::ability(&conn).unwrap(), after_first);
    }

    #[test]
    fn 答满题数后不再出题() {
        let mut conn = db();
        for _ in 0..ITEMS {
            let q = ask(&conn).expect("应有题");
            record_answer(&mut conn, q.word_id, true, 900).unwrap();
        }
        assert!(ask(&conn).is_none(), "答满 {ITEMS} 题后应结束");
    }

    #[test]
    fn 取题会把摸底标记为进行中() {
        let conn = db();
        ask(&conn);
        assert_eq!(
            settings::get(&conn, "placement_stage").unwrap().as_deref(),
            Some("1")
        );
    }

    #[test]
    fn 摸底起点比日常先验弱() {
        let start = ability::Ability::for_placement();
        let daily = ability::Ability::default();
        assert_eq!(start.theta, daily.theta);
        // 摸底期间什么都还没教，波动不付代价——只有「多久摸到真实水平」重要
        assert!(
            start.information < daily.information,
            "摸底先验 {} 应弱于日常先验 {}",
            start.information,
            daily.information
        );
    }

    #[test]
    fn 二十题足以离开先验() {
        let mut conn = db();

        for _ in 0..ITEMS {
            let q = ask(&conn).expect("应有题");
            answer(&mut conn, &q, true);
        }
        let a = player_stats::ability(&conn).unwrap();

        // 纯先验时水平偏离的孩子首场难度必错。摸底的唯一职责就是避免这个
        assert!(
            a.theta > ability::PRIOR_THETA + 1.5,
            "20 题全对后 θ 只到 {:.2}（先验 {:.2}）",
            a.theta,
            ability::PRIOR_THETA
        );
        // 之后的日常更新应当被压住
        assert!(a.information > ability::PRIOR_INFORMATION);
    }
}
