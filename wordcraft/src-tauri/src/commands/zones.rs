//! 区域进度与解锁。spec §5.2。
//!
//! 解锁改为纯等级里程碑（2026-08-07 决议）。spec 原定「清风平原 = 完成新手村」，
//! 但排队算法按 `frequency_band` 取新词、不看 zone——band 1 的 951 个词分散在
//! newbie/grass/water 三个区，用户的学习天然散开，「完成新手村」永远达不成。
//!
//! 改按等级还有一层理由：newbie/grass/water 全是 band 1 高频词，按区上锁会把
//! 蓝水湖泊的 541 个高频词推到很后面，而永冬之巅的低频词反倒不受影响——
//! 与「先学高频词」的效率原则正好相反。

use crate::db::{repo::player_stats, Db};
use rusqlite::Connection;
use serde::Serialize;
use tauri::State;

/// (zone, 显示名, 解锁所需等级)。等级 1 表示默认开放。
///
/// 清风平原定为 Lv.2 而非 spec 的「完成新手村」：新手村 50 词按 6 词/天
/// 要学 8 天，把第二个区锁这么久，地图会在最需要新鲜感的头一周里毫无变化。
const ZONES: [(&str, &str, i64); 6] = [
    ("newbie", "新手村", 1),
    ("grass", "清风平原", 2),
    ("water", "蓝水湖泊", 5),
    ("fire", "赤焰山脉", 15),
    ("thunder", "雷霆峡谷", 25),
    ("ice", "永冬之巅", 40),
];

#[derive(Debug, Serialize)]
pub struct ZoneProgress {
    pub key: String,
    pub name: String,
    /// 该区在**当前学习范围内**的总词数，取自数据库而非硬编码
    pub total: i64,
    /// 真正作答过的词数（`reps > 0`）
    pub learned: i64,
    pub unlocked: bool,
    pub required_level: i64,
}

/// contracts §3.4：各区进度。
///
/// 词数一律现查。前端此前硬编码 200/300/500，而真实词库是 360/541/901——
/// 界面上那几个数字从上线起就是错的，且没有任何机制会发现。
#[tauri::command]
pub fn get_zone_progress(db: State<Db>) -> Result<Vec<ZoneProgress>, String> {
    let conn = db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))?;
    let level = player_stats::get(&conn)?.level;
    // 高中范围下新手村与清风平原全是初中词，会算出 0/0。
    // 前端据此把范围内为空的区隐藏——留着它们等于告诉用户
    // 「这里还有 360 个词要学」，而那些词根本不会被排进队列
    let scope = crate::scope::current(&conn)?.sql_filter();

    ZONES
        .iter()
        .map(|(key, name, required)| {
            let (total, learned) = counts(&conn, key, scope)?;
            Ok(ZoneProgress {
                key: (*key).to_string(),
                name: (*name).to_string(),
                total,
                learned,
                unlocked: level >= *required,
                required_level: *required,
            })
        })
        .collect()
}

fn counts(conn: &Connection, zone: &str, scope_sql: &str) -> Result<(i64, i64), String> {
    // `s.reps > 0` 而非「有状态行」：摸底为一千多个初中词预建了状态，
    // 按行数算会让新手村显示 50/50「已完成」，而用户一个都没练过。
    // 与「已点亮水晶」虚高、与家园方块发放，是同一个判据
    conn.query_row(
        &format!(
            "SELECT COUNT(*),
                    SUM(CASE WHEN s.word_id IS NOT NULL AND s.reps > 0 THEN 1 ELSE 0 END)
             FROM words w LEFT JOIN word_states s ON s.word_id = w.id
             WHERE w.zone = ?1 AND {scope_sql}"
        ),
        [zone],
        |r| Ok((r.get(0)?, r.get::<_, Option<i64>>(1)?.unwrap_or(0))),
    )
    .map_err(|e| format!("统计区域 {zone} 进度失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 解锁等级单调递增() {
        // 后面的区必须更难解锁，否则「地图逐步展开」的观感就乱了
        let levels: Vec<i64> = ZONES.iter().map(|(_, _, l)| *l).collect();
        for pair in levels.windows(2) {
            assert!(pair[0] < pair[1], "解锁等级未递增: {levels:?}");
        }
    }

    #[test]
    fn 新手村默认开放() {
        assert_eq!(ZONES[0].2, 1, "1 级玩家必须能进新手村");
    }

    #[test]
    fn 第二区在头几天内可达() {
        // 新手村 50 词按 6 词/天要 8 天。第二个区若锁到那时，
        // 地图会在最需要新鲜感的头一周毫无变化
        assert!(ZONES[1].2 <= 3, "第二区解锁门槛过高: Lv.{}", ZONES[1].2);
    }

    #[test]
    fn 覆盖全部六个区域() {
        assert_eq!(ZONES.len(), 6);
        let keys: Vec<&str> = ZONES.iter().map(|(k, _, _)| *k).collect();
        for expected in ["newbie", "grass", "water", "fire", "thunder", "ice"] {
            assert!(keys.contains(&expected), "缺少区域 {expected}");
        }
    }
}
