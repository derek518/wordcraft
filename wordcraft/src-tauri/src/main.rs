// Prevent console window in addition to Tauri Window in Windows OS when compiling with Rust.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]


mod db;
mod scheduler;
mod tts;
mod fsrs_engine;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_log::Builder::new().build())
        .setup(|app| {
            let app_handle = app.handle();
            if let Err(e) = db::init_database(&app_handle) {
                eprintln!("Database init error: {}", e);
            }
            scheduler::start_scheduler(app_handle.clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            db::get_due_words,
            db::update_word_review,
            db::get_today_stats,
            db::get_overall_stats,
            db::get_setting,
            db::set_setting,
            db::import_word_library,
            tts::play_word_audio,
            fsrs_engine::get_next_review_words,
            fsrs_engine::submit_review_result,
            scheduler::get_next_session_time,
            scheduler::trigger_popup_now,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
