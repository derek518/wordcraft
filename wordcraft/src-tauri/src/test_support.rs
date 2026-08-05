//! 测试夹具。仅在 `cfg(test)` 下编译。
//!
//! T06/T07 的 migration 与 Repository 测试全部基于此处的内存数据库，
//! 避免测试污染真实数据文件，也避免测试之间互相干扰。

use rusqlite::Connection;

/// 全新的内存 SQLite 连接，启用外键约束。
///
/// 每次调用返回独立数据库——测试之间天然隔离，无需清理。
pub fn in_memory_db() -> Connection {
    let conn = Connection::open_in_memory().expect("打开内存数据库失败");
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .expect("启用外键约束失败");
    conn
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn 内存数据库可建表可读写() {
        let conn = in_memory_db();
        conn.execute_batch(
            "CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
             INSERT INTO t (name) VALUES ('crystal');",
        )
        .expect("建表或插入失败");

        let name: String = conn
            .query_row("SELECT name FROM t WHERE id = 1", [], |r| r.get(0))
            .expect("查询失败");
        assert_eq!(name, "crystal");
    }

    #[test]
    fn 外键约束真实生效() {
        let conn = in_memory_db();
        conn.execute_batch(
            "CREATE TABLE parent (id INTEGER PRIMARY KEY);
             CREATE TABLE child (pid INTEGER REFERENCES parent(id));",
        )
        .unwrap();

        // 指向不存在的父行必须被拒绝——若 PRAGMA 未生效这里会静默成功
        let result = conn.execute("INSERT INTO child (pid) VALUES (999)", []);
        assert!(result.is_err(), "外键约束未生效，测试夹具不可信");
    }

    #[test]
    fn 每次调用返回独立数据库() {
        let a = in_memory_db();
        let b = in_memory_db();
        a.execute_batch("CREATE TABLE only_in_a (x INTEGER);").unwrap();

        let leaked = b
            .prepare("SELECT 1 FROM only_in_a")
            .is_ok();
        assert!(!leaked, "两个连接共享了同一数据库，测试无法隔离");
    }

    /// chrono 可用性冒烟测试。
    ///
    /// 审计 D2：此前项目移除 chrono 后手写日历运算，导致 85% 的日期算错
    /// （`86464` 秒/天、忽略闰年、按 30 天分月）。ADR-4 禁止手写日期运算。
    #[test]
    fn chrono_正确处理闰年与月长() {
        // 2024 是闰年，2 月有 29 天
        let leap = Utc.with_ymd_and_hms(2024, 2, 29, 0, 0, 0).unwrap();
        assert_eq!(leap.format("%Y-%m-%d").to_string(), "2024-02-29");

        // 手写实现按 30 天分月，无法表示 31 号
        let long_month = Utc.with_ymd_and_hms(2026, 8, 31, 0, 0, 0).unwrap();
        assert_eq!(long_month.format("%Y-%m-%d").to_string(), "2026-08-31");

        // 跨月加天数：8 月 31 日 + 1 天 = 9 月 1 日
        let next = long_month + chrono::Duration::days(1);
        assert_eq!(next.format("%Y-%m-%d").to_string(), "2026-09-01");
    }
}
