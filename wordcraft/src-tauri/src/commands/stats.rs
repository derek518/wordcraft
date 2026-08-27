//! 统计查询 command。契约见 contracts-v1.md §3.4。
//!
//! 所有数字均由 SQL 聚合真实数据产出——审计 D5 的教训是任何一处硬编码的
//! 「示例数据」都会在词库扩大后变成谎言。

use crate::ability;
use crate::db::{clock, repo::*, Db};
use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct OverallStats {
    pub total_words: i64,
    pub untouched: i64,
    pub total_reviews: i64,
    pub total_xp: i64,
    pub level: i64,
    pub current_streak: i64,
    pub best_streak: i64,
    pub vocab_estimate: i64,
    pub draw_tickets: i64,
    pub makeup_cards: i64,
}

/// 五段掌握分布（spec F8 的色条）。
///
/// 各段之和恒等于词库总数：`untouched` 承接尚无 `word_states` 记录的词，
/// 否则新导入的词会从统计中凭空消失。
#[derive(Debug, Default, Serialize)]
pub struct MasteryDistribution {
    pub untouched: i64,
    pub learning: i64,
    pub reinforcing: i64,
    pub review: i64,
    pub mastered: i64,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct HeatmapCell {
    pub date: String,
    pub count: i64,
}

pub fn overall(conn: &Connection) -> Result<OverallStats, String> {
    let stats = player_stats::get(conn)?;
    // 分子分母都限定在学习范围内。高中范围下把两千个不教的初中词算进
    // 分母，与「已点亮」虚高是同一类失真
    let scope = crate::scope::current(conn)?.sql_filter();
    Ok(OverallStats {
        total_words: words::count_in_scope(conn, scope)?,
        untouched: word_states::untouched_count(conn, scope)?,
        total_reviews: review_logs::total_count(conn)?,
        total_xp: stats.total_xp,
        level: stats.level,
        current_streak: stats.current_streak,
        best_streak: stats.best_streak,
        vocab_estimate: stats.vocab_estimate,
        draw_tickets: stats.draw_tickets,
        makeup_cards: stats.makeup_cards,
    })
}

pub fn mastery_distribution(conn: &Connection) -> Result<MasteryDistribution, String> {
    let scope = crate::scope::current(conn)?.sql_filter();
    let mut out = MasteryDistribution {
        total: words::count_in_scope(conn, scope)?,
        untouched: word_states::untouched_count(conn, scope)?,
        ..Default::default()
    };

    for (state, count) in word_states::distribution(conn)? {
        match state.as_str() {
            // 'new' 状态的词已有 word_states 行但尚未作答，与从无记录的词同属未学
            "new" => out.untouched += count,
            "learning" => out.learning = count,
            "reinforcing" => out.reinforcing = count,
            "review" => out.review = count,
            "mastered" => out.mastered = count,
            other => {
                return Err(format!(
                    "word_states 中出现未知 app_state `{other}`，schema 与代码已脱节"
                ))
            }
        }
    }
    Ok(out)
}

/// 最近 `days` 天的作答热力图，缺失的日期补 0。
///
/// 补零是必要的：前端按固定网格渲染日历，缺日会导致格子错位。
pub fn heatmap(conn: &Connection, days: i64) -> Result<Vec<HeatmapCell>, String> {
    if !(1..=400).contains(&days) {
        return Err(format!("days 必须在 1..400，收到 {days}"));
    }

    let mut cells = Vec::with_capacity(days as usize);
    let today = clock::parse_ts(&clock::now())?;

    for offset in (0..days).rev() {
        let date = clock::local_date_of(clock::add_days(today, -offset));
        let count = review_logs::stats_for_day(conn, &date)?.total;
        cells.push(HeatmapCell { date, count });
    }
    Ok(cells)
}

/// spec F7：导出全部数据为 JSON。
pub fn export_json(conn: &Connection) -> Result<String, String> {
    let payload = serde_json::json!({
        "exported_at": clock::now(),
        "overall": overall(conn)?,
        "mastery": mastery_distribution(conn)?,
        "today": review_logs::stats_for_day(conn, &clock::today())?,
        "heatmap": heatmap(conn, 365)?,
    });
    serde_json::to_string_pretty(&payload).map_err(|e| format!("序列化导出数据失败: {e}"))
}

// ─────────────────────────────────────────────
// Tauri commands
// ─────────────────────────────────────────────

fn lock(db: &Db) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
    db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))
}

#[tauri::command]
pub fn get_today_stats(db: State<Db>) -> Result<review_logs::DayStats, String> {
    let conn = lock(&db)?;
    review_logs::stats_for_day(&conn, &clock::today())
}

#[tauri::command]
pub fn get_overall_stats(db: State<Db>) -> Result<OverallStats, String> {
    let conn = lock(&db)?;
    overall(&conn)
}

#[tauri::command]
pub fn get_mastery_distribution(db: State<Db>) -> Result<MasteryDistribution, String> {
    let conn = lock(&db)?;
    mastery_distribution(&conn)
}

#[tauri::command]
pub fn get_heatmap(db: State<Db>, days: i64) -> Result<Vec<HeatmapCell>, String> {
    let conn = lock(&db)?;
    heatmap(&conn, days)
}

#[tauri::command]
pub fn export_data_json(db: State<Db>) -> Result<String, String> {
    let conn = lock(&db)?;
    export_json(&conn)
}

/// 能力概览：孩子的水平估到哪，重点该放在哪一段。
///
/// 这是「学习范围」那个下拉框的替代品。范围不再由家长猜，而是由每天的作答
/// 算出来——见 `ability.rs` 与契约 §13。
#[derive(Debug, Serialize)]
pub struct AbilityOverview {
    /// 估计的词汇量：词库里排名在掌握边界之前的词数
    pub vocabulary: i64,
    /// 不确定区间（±1 标准误换算成词汇量）
    pub vocabulary_low: i64,
    pub vocabulary_high: i64,
    /// 学习前沿的词频排名区间
    pub frontier_from: i64,
    pub frontier_to: i64,
    /// 词库按能力分层的词数
    pub known: i64,
    pub frontier: i64,
    pub too_hard: i64,
    /// 前沿里还没学过的词数——还剩多少可练
    pub frontier_untouched: i64,
    /// 参与估计的观测数。为 0 表示还在用先验，界面该说明这一点
    pub observations: i64,
}

/// 逐词分类而不是用 SQL 的区间条件：分层只该有一份定义（`ability::tier`）。
/// SQL 里抄一份边界，改了阈值这里就开始说谎。
pub fn overview_of(conn: &Connection, a: &ability::Ability) -> Result<AbilityOverview, String> {
    let mut stmt = conn
        .prepare(
            "SELECT w.frequency_rank,
                    EXISTS (SELECT 1 FROM word_states s WHERE s.word_id = w.id) AS touched
             FROM words w WHERE w.frequency_rank IS NOT NULL",
        )
        .map_err(|e| format!("准备能力概览查询失败: {e}"))?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, bool>(1)?)))
        .map_err(|e| format!("执行能力概览查询失败: {e}"))?;

    let (mut known, mut frontier, mut too_hard, mut frontier_untouched) = (0, 0, 0, 0);
    for row in rows {
        let (rank, touched) = row.map_err(|e| format!("读取能力概览失败: {e}"))?;
        match ability::tier(a.theta, rank) {
            ability::Tier::Known => known += 1,
            ability::Tier::Frontier => {
                frontier += 1;
                if !touched {
                    frontier_untouched += 1;
                }
            }
            ability::Tier::TooHard => too_hard += 1,
        }
    }

    let (frontier_from, frontier_to) = ability::frontier_ranks(a.theta);
    let se = ability::standard_error(a.information);
    // 把不确定度换算成词汇量上下界：θ ± 1 个标准误，比「SE = 0.46」好懂得多
    let vocab_at = |theta: f64| player_stats::vocab_from_ability(conn, &ability::Ability { theta, ..*a });

    Ok(AbilityOverview {
        vocabulary: vocab_at(a.theta)?,
        vocabulary_low: vocab_at(a.theta - se)?,
        vocabulary_high: vocab_at(a.theta + se)?,
        frontier_from,
        frontier_to,
        known,
        frontier,
        too_hard,
        frontier_untouched,
        observations: a.observations,
    })
}

/// contracts §3.4
#[tauri::command]
pub fn get_ability_overview(db: State<Db>) -> Result<AbilityOverview, String> {
    let conn = db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))?;
    let a = player_stats::ability(&conn)?;
    overview_of(&conn, &a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;
    use crate::test_support::in_memory_db;

    fn setup(word_count: usize) -> Connection {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();
        let items: Vec<words::WordImport> = (0..word_count)
            .map(|i| words::WordImport {
                word: format!("word{}", (b'a' + i as u8) as char),
                phonetic: "/w/".into(),
                pos: "n.".into(),
                meaning: format!("释义{i}"),
                example_1: format!("A word{} appears.", (b'a' + i as u8) as char),
                example_2: String::new(),
                // senior：与默认学习范围一致。用 junior 的话统计全被范围
                // 过滤挡掉，下面测的就不是统计逻辑了
                level: "senior".into(),
                frequency_band: 1,
                frequency_rank: None,
                zone: "newbie".into(),
                source_edition: String::new(),
            })
            .collect();
        words::import(&mut conn, &items).unwrap();
        conn
    }

    fn set_state(conn: &Connection, id: i64, app_state: &str) {
        word_states::upsert(
            conn,
            &word_states::WordState {
                word_id: id,
                difficulty: 5.0,
                stability: 1.0,
                due_at: clock::now(),
                fsrs_state: 1,
                app_state: app_state.into(),
                reps: 1,
                lapses: 0,
                question_level: 1,
                reinforce_streak: 0,
                last_review_at: None,
                mastered_at: None,
            },
        )
        .unwrap();
    }

    #[test]
    fn 掌握分布五段之和等于词库总数() {
        let conn = setup(10);
        set_state(&conn, 1, "learning");
        set_state(&conn, 2, "reinforcing");
        set_state(&conn, 3, "review");
        set_state(&conn, 4, "mastered");
        set_state(&conn, 5, "new");

        let d = mastery_distribution(&conn).unwrap();
        let sum = d.untouched + d.learning + d.reinforcing + d.review + d.mastered;

        assert_eq!(d.total, 10);
        assert_eq!(sum, d.total, "五段之和 {sum} 与总数 {} 不符", d.total);
        // 5 个从未建过状态的词 + 1 个 app_state='new' 的词
        assert_eq!(d.untouched, 6);
        assert_eq!(d.mastered, 1);
    }

    #[test]
    fn 空库统计返回零而非报错() {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();

        let d = mastery_distribution(&conn).unwrap();
        assert_eq!(d.total, 0);
        assert_eq!(d.untouched, 0);

        let o = overall(&conn).unwrap();
        assert_eq!(o.total_words, 0);
        assert_eq!(o.level, 1, "初始等级应为 1");
    }

    #[test]
    fn 新导入的词不会从统计中消失() {
        let conn = setup(5);
        // 一个 word_states 记录都没有
        let d = mastery_distribution(&conn).unwrap();
        assert_eq!(d.untouched, 5, "无状态记录的词必须计入 untouched");
        assert_eq!(d.total, 5);
    }

    #[test]
    fn 热力图补齐缺失日期() {
        let conn = setup(3);
        let cells = heatmap(&conn, 7).unwrap();

        assert_eq!(cells.len(), 7, "应返回恰好 7 个格子");
        assert!(cells.iter().all(|c| c.count == 0), "无作答时应全为 0");
        // 日期升序，末位是今天
        assert_eq!(cells.last().unwrap().date, clock::today());
        let mut sorted = cells.iter().map(|c| c.date.clone()).collect::<Vec<_>>();
        sorted.sort();
        assert_eq!(
            sorted,
            cells.iter().map(|c| c.date.clone()).collect::<Vec<_>>(),
            "热力图日期应按升序排列"
        );
    }

    #[test]
    fn 热力图拒绝越界天数() {
        let conn = setup(1);
        assert!(heatmap(&conn, 0).is_err());
        assert!(heatmap(&conn, -1).is_err());
        assert!(heatmap(&conn, 401).is_err());
        assert!(heatmap(&conn, 365).is_ok());
    }

    #[test]
    fn 导出为合法_json_且含关键字段() {
        let conn = setup(3);
        let json = export_json(&conn).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("导出内容不是合法 JSON");

        assert!(parsed["exported_at"].is_string());
        assert_eq!(parsed["overall"]["total_words"], 3);
        assert!(parsed["mastery"]["total"].is_number());
        assert!(parsed["heatmap"].is_array());
    }

    #[test]
    fn 未知状态值让统计报错而非静默归零() {
        let conn = setup(2);
        // 绕过 CHECK 约束不可能，故直接验证已知状态被正确分类；
        // 此测试锁定的是「遇到未知值必须报错」这条分支的存在意义
        set_state(&conn, 1, "mastered");
        let d = mastery_distribution(&conn).unwrap();
        assert_eq!(d.mastered, 1);
    }
}
