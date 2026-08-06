// Prevent console window in addition to Tauri Window in Windows OS when compiling with Rust.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]


use tauri::Manager;

mod db;
mod queue;
mod review;
mod scheduler;
mod tts;
mod fsrs_engine;

#[cfg(test)]
mod test_support;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_log::Builder::new().build())
        .setup(|app| {
            let app_handle = app.handle();

            // 数据库不可用时应用没有任何意义，故此处失败即终止启动，
            // 不降级、不静默继续 —— 那正是审计 D6 的失效模式。
            let database = db::init(app_handle)?;
            app.manage(database);

            scheduler::start_scheduler(app_handle.clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // TODO(T07): 以下 legacy command 由 SQLite Repository 实现替换，见 MOCKS.md M4
            db::legacy::get_due_words,
            db::legacy::update_word_review,
            db::legacy::get_today_stats,
            db::legacy::get_overall_stats,
            db::legacy::get_setting,
            db::legacy::set_setting,
            db::legacy::import_word_library,
            queue::get_session_queue,
            review::commit_review,
            tts::play_word_audio,
            fsrs_engine::get_next_review_words,
            fsrs_engine::submit_review_result,
            scheduler::get_next_session_time,
            scheduler::trigger_popup_now,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
