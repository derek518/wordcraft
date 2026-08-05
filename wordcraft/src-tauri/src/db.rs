use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri::Manager;
use std::fs;
use std::path::PathBuf;

const DATA_FILE: &str = "wordcraft_data.json";

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AppData {
    pub words: Vec<Word>,
    pub word_states: Vec<WordState>,
    pub review_logs: Vec<ReviewLog>,
    pub player_stats: PlayerStats,
    pub settings: std::collections::HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Word {
    pub id: i64,
    pub word: String,
    pub phonetic: String,
    pub meaning: String,
    pub pos: String,
    pub example_1: String,
    pub example_2: String,
    pub level: String,
    pub frequency_band: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WordState {
    pub word_id: i64,
    pub state: String,
    pub due_date: String,
    pub last_review: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReviewLog {
    pub word_id: i64,
    pub result: i32,
    pub reaction_ms: i64,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlayerStats {
    pub total_xp: i64,
    pub level: i64,
    pub current_streak: i64,
    pub best_streak: i64,
}

impl Default for PlayerStats {
    fn default() -> Self {
        PlayerStats {
            total_xp: 0,
            level: 1,
            current_streak: 0,
            best_streak: 0,
        }
    }
}

fn get_data_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir()
        .map(|dir| dir.join(DATA_FILE))
        .map_err(|e| e.to_string())
}

fn load_data(app: &AppHandle) -> Result<AppData, String> {
    let path = get_data_path(app)?;
    if path.exists() {
        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    } else {
        Ok(AppData::default())
    }
}

fn save_data(app: &AppHandle, data: &AppData) -> Result<(), String> {
    let path = get_data_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
    fs::write(&path, content).map_err(|e| e.to_string())
}

pub fn init_database(app: &AppHandle) -> Result<(), String> {
    let mut data = load_data(app)?;
    
    if data.settings.is_empty() {
        data.settings.insert("first_run".to_string(), "true".to_string());
        data.settings.insert("sound_enabled".to_string(), "true".to_string());
        save_data(app, &data)?;
    }
    
    Ok(())
}

#[tauri::command]
pub fn get_due_words(app: AppHandle, limit: i64, _session_type: String) -> Result<Vec<Word>, String> {
    let data = load_data(&app)?;
    let mut result = Vec::new();
    
    for word in &data.words {
        let state = data.word_states.iter().find(|s| s.word_id == word.id);
        if state.is_none() || state.unwrap().state == "new" {
            result.push(word.clone());
        }
        if result.len() as i64 >= limit {
            break;
        }
    }
    
    Ok(result)
}

#[tauri::command]
pub fn update_word_review(
    app: AppHandle,
    word_id: i64,
    result: i32,
    reaction_ms: i64,
    _question_type: i32,
) -> Result<(), String> {
    let mut data = load_data(&app)?;
    let now = current_timestamp();
    
    data.review_logs.push(ReviewLog {
        word_id,
        result,
        reaction_ms,
        timestamp: now.clone(),
    });
    
    let state = data.word_states.iter_mut().find(|s| s.word_id == word_id);
    let new_state = match result {
        3 => ("review", add_days(&now, 7)),
        2 => ("review", add_days(&now, 3)),
        1 => ("learning", add_days(&now, 1)),
        _ => ("relearning", add_days(&now, 1)),
    };
    
    if let Some(s) = state {
        s.state = new_state.0.to_string();
        s.due_date = new_state.1;
        s.last_review = Some(now);
    } else {
        data.word_states.push(WordState {
            word_id,
            state: new_state.0.to_string(),
            due_date: new_state.1,
            last_review: Some(now),
        });
    }
    
    save_data(&app, &data)
}

#[tauri::command]
pub fn get_today_stats(app: AppHandle) -> Result<serde_json::Value, String> {
    let data = load_data(&app)?;
    let today = today_str();
    
    let mut total = 0i64;
    let mut again = 0i64;
    let mut hard = 0i64;
    let mut good = 0i64;
    let mut easy = 0i64;
    
    for log in &data.review_logs {
        if log.timestamp.starts_with(&today) {
            total += 1;
            match log.result {
                0 => again += 1,
                1 => hard += 1,
                2 => good += 1,
                3 => easy += 1,
                _ => {}
            }
        }
    }
    
    Ok(serde_json::json!({
        "total": total,
        "again": again,
        "hard": hard,
        "good": good,
        "easy": easy,
    }))
}

#[tauri::command]
pub fn get_overall_stats(app: AppHandle) -> Result<serde_json::Value, String> {
    let data = load_data(&app)?;
    
    let total_words = data.words.len() as i64;
    let new_words = data.word_states.iter().filter(|s| s.state == "new" || s.state == "").count() as i64;
    let learning = data.word_states.iter().filter(|s| s.state == "learning" || s.state == "relearning").count() as i64;
    let review = data.word_states.iter().filter(|s| s.state == "review").count() as i64;
    let mastered = data.word_states.iter().filter(|s| s.state == "mastered").count() as i64;
    let total_reviews = data.review_logs.len() as i64;
    
    Ok(serde_json::json!({
        "total_words": total_words,
        "new_words": new_words,
        "learning": learning,
        "review": review,
        "mastered": mastered,
        "total_reviews": total_reviews,
        "current_streak": data.player_stats.current_streak,
        "best_streak": data.player_stats.best_streak,
        "total_xp": data.player_stats.total_xp,
        "level": data.player_stats.level,
    }))
}

#[tauri::command]
pub fn get_setting(app: AppHandle, key: String) -> Result<Option<String>, String> {
    let data = load_data(&app)?;
    Ok(data.settings.get(&key).cloned())
}

#[tauri::command]
pub fn set_setting(app: AppHandle, key: String, value: String) -> Result<(), String> {
    let mut data = load_data(&app)?;
    data.settings.insert(key, value);
    save_data(&app, &data)
}

#[tauri::command]
pub fn import_word_library(app: AppHandle, words: Vec<serde_json::Value>) -> Result<i64, String> {
    let mut data = load_data(&app)?;
    let mut inserted = 0i64;
    let start_id = data.words.len() as i64 + 1;
    
    for (i, w) in words.iter().enumerate() {
        let word = Word {
            id: start_id + i as i64,
            word: w.get("word").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            phonetic: w.get("phonetic").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            meaning: w.get("meaning").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            pos: w.get("pos").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            example_1: w.get("example_1").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            example_2: w.get("example_2").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            level: w.get("level").and_then(|v| v.as_str()).unwrap_or("junior").to_string(),
            frequency_band: w.get("frequency_band").and_then(|v| v.as_i64()).unwrap_or(1),
        };
        
        if !data.words.iter().any(|existing| existing.word == word.word) {
            data.words.push(word);
            inserted += 1;
        }
    }
    
    save_data(&app, &data)?;
    Ok(inserted)
}

fn current_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let secs_per_day = 86464u64;
    let days_since_epoch = now / secs_per_day;
    let year = 1970i64 + (days_since_epoch / 365) as i64;
    let day_of_year = days_since_epoch % 365;
    let month = (day_of_year / 30) + 1;
    let day = (day_of_year % 30) + 1;
    format!("{:04}-{:02}-{:02} 00:00:00", year, month, day)
}

fn today_str() -> String {
    current_timestamp().split_whitespace().next().unwrap_or("").to_string()
}

fn add_days(date_str: &str, _days: i64) -> String {
    let parts: Vec<&str> = date_str.split_whitespace().collect();
    parts[0].to_string()
}
