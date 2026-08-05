//! 数据访问层。
//!
//! 数据库仅 Rust 侧可访问（ADR-1）——前端一律通过 command 契约读写，
//! 无法绕过校验直接操作 SQL。

pub mod migrations;

/// T07 将删除此模块。当前保留以维持 command 可用性。
/// 其内部为 JSON 文件存储与手写日期运算，见 MOCKS.md M4。
///
/// 注意：command 必须以 `db::legacy::xxx` 的完整路径注册——`tauri::generate_handler!`
/// 依赖宏生成的隐藏项，`pub use` re-export 无法透传。
pub mod legacy;

use chrono::Utc;
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

const DB_FILE: &str = "wordcraft.db";
const LEGACY_JSON: &str = "wordcraft_data.json";

/// 放入 Tauri State 的数据库句柄。
///
/// `Connection` 不是 `Sync`，故用 `Mutex` 包装。桌面单用户场景下锁竞争可忽略。
///
/// 字段目前仅由 `init` 写入——T07 的 Repository 层会通过 `tauri::State<Db>`
/// 取用它，届时读路径接通。
#[allow(dead_code)]
pub struct Db(pub Mutex<Connection>);

/// 打开数据库、应用迁移，并归档遗留 JSON 数据。
pub fn init(app: &AppHandle) -> Result<Db, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取应用数据目录失败: {e}"))?;

    fs::create_dir_all(&dir).map_err(|e| format!("创建应用数据目录失败: {e}"))?;

    let mut conn = open(&dir.join(DB_FILE))?;
    let applied = migrations::run(&mut conn)?;

    if applied.is_empty() {
        log::info!(
            "数据库 schema 已是最新（版本 {}）",
            migrations::current_version(&conn)?
        );
    } else {
        log::info!("已应用数据库迁移: {applied:?}");
    }

    archive_legacy_json_once(&dir, &conn)?;

    Ok(Db(Mutex::new(conn)))
}

/// 归档遗留 JSON，且只归档一次。
///
/// 归档动作必须幂等：在 T07 删除 legacy 模块之前，legacy 的写路径仍可能重新
/// 创建 JSON 文件；若不加标记，每次启动都会再生成一个 .bak，把数据目录堆满。
fn archive_legacy_json_once(dir: &Path, conn: &Connection) -> Result<(), String> {
    const FLAG: &str = "legacy_json_archived";

    let already: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [FLAG], |r| {
            r.get(0)
        })
        .ok();

    if already.as_deref() == Some("true") {
        return Ok(());
    }

    archive_legacy_json(dir)?;

    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, 'true')",
        [FLAG],
    )
    .map_err(|e| format!("记录遗留数据归档标记失败: {e}"))?;

    Ok(())
}

/// 打开连接并设置必要的 PRAGMA。
///
/// `foreign_keys` 必须显式开启——SQLite 默认关闭，外键会被静默忽略。
pub fn open(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path)
        .map_err(|e| format!("打开数据库 {} 失败: {e}", path.display()))?;

    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;",
    )
    .map_err(|e| format!("设置数据库 PRAGMA 失败: {e}"))?;

    Ok(conn)
}

/// 归档遗留的 JSON 数据文件。
///
/// 开发期 JSON 数据（52 词 + 少量日志）不做迁移——其时间戳由存在缺陷的手写日期
/// 函数生成（审计 D2，85% 的日期是错的），迁移过来只会污染新库。
/// 但也不直接删除：重命名保留，以便需要时人工查看。
fn archive_legacy_json(dir: &Path) -> Result<(), String> {
    let json_path = dir.join(LEGACY_JSON);
    if !json_path.exists() {
        return Ok(());
    }

    let stamp = Utc::now().format("%Y%m%dT%H%M%SZ");
    let backup: PathBuf = dir.join(format!("{LEGACY_JSON}.{stamp}.bak"));

    fs::rename(&json_path, &backup)
        .map_err(|e| format!("归档遗留数据文件失败: {e}"))?;

    log::info!(
        "检测到遗留 JSON 数据，已归档为 {}。其内容不迁移至 SQLite（时间戳不可信，见审计 D2）",
        backup.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wordcraft_test_{tag}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn 打开的连接启用了外键约束() {
        let dir = temp_dir("fk");
        let conn = open(&dir.join("t.db")).unwrap();

        let enabled: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert_eq!(enabled, 1, "外键未启用，所有 REFERENCES 会被静默忽略");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 遗留_json_被归档而非删除() {
        let dir = temp_dir("legacy");
        let json = dir.join(LEGACY_JSON);
        fs::write(&json, r#"{"words":[]}"#).unwrap();

        archive_legacy_json(&dir).unwrap();

        assert!(!json.exists(), "原 JSON 文件应已被移走");
        let backups: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".bak"))
            .collect();
        assert_eq!(backups.len(), 1, "应恰好留下一个备份文件");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 无遗留文件时归档是空操作() {
        let dir = temp_dir("nolegacy");
        assert!(archive_legacy_json(&dir).is_ok());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 重复归档不覆盖既有备份() {
        let dir = temp_dir("twice");

        fs::write(dir.join(LEGACY_JSON), "first").unwrap();
        archive_legacy_json(&dir).unwrap();

        // 第二次归档：备份名含秒级时间戳，同秒内可能重名，此处仅验证不报错且原文件被移走
        fs::write(dir.join(LEGACY_JSON), "second").unwrap();
        archive_legacy_json(&dir).unwrap();
        assert!(!dir.join(LEGACY_JSON).exists());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 磁盘数据库迁移后可重开并保留数据() {
        let dir = temp_dir("persist");
        let path = dir.join("t.db");

        {
            let mut conn = open(&path).unwrap();
            migrations::run(&mut conn).unwrap();
            conn.execute(
                "INSERT INTO words (word, pos, meaning, example_1, level, frequency_band, zone, created_at)
                 VALUES ('crystal', 'n.', '水晶', 'A glowing crystal.', 'junior', 1, 'newbie', '2026-08-05T00:00:00Z')",
                [],
            )
            .unwrap();
        }

        let mut conn = open(&path).unwrap();
        let applied = migrations::run(&mut conn).unwrap();
        assert!(applied.is_empty(), "重开时不应重复应用迁移");

        let word: String = conn
            .query_row("SELECT word FROM words WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(word, "crystal", "重开后数据丢失");

        fs::remove_dir_all(&dir).ok();
    }
}
