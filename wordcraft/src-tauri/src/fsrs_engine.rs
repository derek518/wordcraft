use serde::{Deserialize, Serialize};
use tauri::AppHandle;

#[derive(Debug, Serialize, Deserialize)]
pub struct WordReview {
    pub word_id: i64,
    pub word: String,
    pub meaning: String,
    pub phonetic: String,
    pub state: String,
    pub question_type: i32,
    pub options: Vec<String>,
}

#[tauri::command]
pub fn get_next_review_words(
    app: AppHandle,
    count: i64,
) -> Result<Vec<WordReview>, String> {
    use crate::db::legacy::*;
    
    let words = get_due_words(app, count, "review".to_string())?;
    
    let mut reviews = Vec::new();
    for word in words {
        let options = generate_options(&word.meaning);
        reviews.push(WordReview {
            word_id: word.id,
            word: word.word.clone(),
            meaning: word.meaning,
            phonetic: word.phonetic,
            state: "learning".to_string(),
            question_type: 1,
            options,
        });
    }
    
    Ok(reviews)
}

#[tauri::command]
pub fn submit_review_result(
    app: AppHandle,
    word_id: i64,
    rating: i32,
    reaction_ms: i64,
) -> Result<serde_json::Value, String> {
    use crate::db::legacy::*;
    
    let result = match rating {
        4 => 3,
        3 => 2,
        2 => 1,
        _ => 0,
    };
    
    update_word_review(app.clone(), word_id, result, reaction_ms, 1)?;
    
    let xp = calculate_xp(rating);
    
    Ok(serde_json::json!({
        "xp": xp,
        "rating": rating,
    }))
}

fn generate_options(_correct: &str) -> Vec<String> {
    vec![
        "选项A".to_string(),
        "选项B".to_string(),
        "选项C".to_string(),
        "选项D".to_string(),
    ]
}

fn calculate_xp(rating: i32) -> i64 {
    match rating {
        4 => 15,
        3 => 10,
        2 => 5,
        _ => 1,
    }
}
