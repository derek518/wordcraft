//! 词库 command。契约见 contracts-v1.md §3.1。

use crate::db::{repo::words, Db};
use tauri::State;

/// 批量导入。校验失败的词条被拒绝并回报原因，**不静默跳过**——
/// 导入 5000 词时静默丢掉 300 条，直到用户发现某个词永远不出现才会暴露。
#[tauri::command]
pub fn import_words(
    db: State<Db>,
    payload: Vec<words::WordImport>,
) -> Result<words::ImportOutcome, String> {
    let mut conn = db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))?;
    words::import(&mut conn, &payload)
}

#[tauri::command]
pub fn search_words(db: State<Db>, keyword: String, limit: i64) -> Result<Vec<words::Word>, String> {
    if keyword.trim().is_empty() {
        return Ok(Vec::new());
    }
    if !(1..=200).contains(&limit) {
        return Err(format!("limit 必须在 1..200，收到 {limit}"));
    }
    let conn = db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))?;
    words::search(&conn, keyword.trim(), limit)
}

/// 干扰项候选池，contracts §6。
///
/// `question_level` 同时决定两件事：返回释义还是单词（Lv.1 选中文、Lv.2+ 选英文），
/// 以及候选的相似度排序策略。词性、频段等条件由后端从 `word_id` 自行查出，
/// 前端不必了解挑选规则。
#[tauri::command]
pub fn get_distractor_pool(
    db: State<Db>,
    word_id: i64,
    question_level: i64,
    count: i64,
) -> Result<Vec<String>, String> {
    if !(1..=50).contains(&count) {
        return Err(format!("count 必须在 1..50，收到 {count}"));
    }
    if !(1..=5).contains(&question_level) {
        return Err(format!("question_level 必须在 1..5，收到 {question_level}"));
    }
    let conn = db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))?;
    words::distractor_pool(&conn, word_id, question_level, count)
}
