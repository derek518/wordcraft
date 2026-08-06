// Prevent console window in addition to Tauri Window in Windows OS when compiling with Rust.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]


use tauri::Manager;

mod commands;
mod db;
mod progression;
mod queue;
mod review;
mod scheduler;
mod tts;

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
            progression::run_daily_rollover(&database)?;
            app.manage(database);

            scheduler::start_scheduler(app_handle.clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 词库与排队
            commands::library::import_words,
            commands::library::search_words,
            commands::library::get_distractor_pool,
            queue::get_session_queue,
            // 作答
            review::commit_review,
            // 会话生命周期
            commands::session::start_session,
            commands::session::finish_session,
            commands::session::get_today_sessions,
            commands::session::postpone_session,
            commands::session::mark_session_eligible,
            commands::session::get_daily_record,
            commands::session::activate_daily_pause,
            progression::settle_day,
            // 统计
            commands::stats::get_today_stats,
            commands::stats::get_overall_stats,
            commands::stats::get_mastery_distribution,
            commands::stats::get_heatmap,
            commands::stats::export_data_json,
            // 设置
            commands::config::get_setting,
            commands::config::set_setting,
            // 平台能力
            tts::play_word_audio,
            scheduler::get_next_session_time,
            scheduler::trigger_popup_now,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
