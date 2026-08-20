//! 设置读写。键契约见 contracts-v1.md §2.1。

use rusqlite::{Connection, OptionalExtension};

/// 延后到期时刻。与 `postpone_session_type` 成对出现。
pub const POSTPONE_UNTIL: &str = "postpone_until";
/// 正在延后的时段类型。
pub const POSTPONE_TYPE: &str = "postpone_session_type";

pub fn get(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
        r.get(0)
    })
    .optional()
    .map_err(|e| format!("读取设置 `{key}` 失败: {e}"))
}

pub fn set(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )
    .map_err(|e| format!("写入设置 `{key}` 失败: {e}"))?;
    Ok(())
}

/// 读取整数设置。值缺失或无法解析时返回 `default` 并记 warn。
///
/// 不因一个损坏的设置项让整个应用起不来，但也不静默——日志里必须留痕。
pub fn get_int(conn: &Connection, key: &str, default: i64) -> Result<i64, String> {
    match get(conn, key)? {
        None => Ok(default),
        Some(raw) => match raw.parse::<i64>() {
            Ok(v) => Ok(v),
            Err(_) => {
                log::warn!("设置 `{key}` 的值 `{raw}` 不是整数，回退到默认值 {default}");
                Ok(default)
            }
        },
    }
}

pub fn get_bool(conn: &Connection, key: &str, default: bool) -> Result<bool, String> {
    match get(conn, key)? {
        None => Ok(default),
        Some(raw) => match raw.as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            other => {
                log::warn!("设置 `{key}` 的值 `{other}` 不是布尔值，回退到默认值 {default}");
                Ok(default)
            }
        },
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
    fn 迁移写入的默认值可读() {
        let conn = db();
        assert_eq!(get(&conn, "daily_new_words").unwrap().as_deref(), Some("6"));
        assert_eq!(get(&conn, "tts_provider").unwrap().as_deref(), Some("edge"));
    }

    #[test]
    fn 不存在的键返回_none() {
        let conn = db();
        assert_eq!(get(&conn, "没有这个键").unwrap(), None);
    }

    #[test]
    fn 写入后可读且重复写入为更新而非报错() {
        let conn = db();
        set(&conn, "daily_new_words", "4").unwrap();
        assert_eq!(get(&conn, "daily_new_words").unwrap().as_deref(), Some("4"));

        set(&conn, "daily_new_words", "8").unwrap();
        assert_eq!(get(&conn, "daily_new_words").unwrap().as_deref(), Some("8"));

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM settings WHERE key = 'daily_new_words'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "重复写入不应产生多行");
    }

    #[test]
    fn 整数解析失败回退默认值() {
        let conn = db();
        set(&conn, "daily_new_words", "六个").unwrap();
        assert_eq!(get_int(&conn, "daily_new_words", 6).unwrap(), 6);
    }

    #[test]
    fn 布尔值接受多种写法() {
        let conn = db();
        for (raw, expected) in [("true", true), ("1", true), ("false", false), ("0", false)] {
            set(&conn, "sound_enabled", raw).unwrap();
            assert_eq!(get_bool(&conn, "sound_enabled", true).unwrap(), expected);
        }

        set(&conn, "sound_enabled", "yes").unwrap();
        assert!(get_bool(&conn, "sound_enabled", true).unwrap(), "非法值应回退到默认值");
    }

    #[test]
    fn 缺失键取默认值() {
        let conn = db();
        assert_eq!(get_int(&conn, "不存在", 42).unwrap(), 42);
        assert!(!get_bool(&conn, "不存在", false).unwrap());
    }
}
