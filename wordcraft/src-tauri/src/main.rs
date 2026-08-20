// Prevent console window in addition to Tauri Window in Windows OS when compiling with Rust.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]


use tauri::Manager;

mod boss;
mod cards;
mod commands;
mod db;
mod homestead;
mod placement;
mod platform;
mod progression;
mod queue;
mod report;
mod review;
mod scheduler;
mod season;
mod tray;
mod tts;

#[cfg(test)]
mod test_support;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_log::Builder::new().build())
        .setup(|app| {
            let app_handle = app.handle();

            // 数据库不可用时应用没有任何意义，故此处失败即终止启动，
            // 不降级、不静默继续 —— 那正是审计 D6 的失效模式。
            let database = db::init(app_handle)?;
            progression::run_daily_rollover(&database)?;
            // 家园方块补发。放在 rollover 之后——streak 结算可能刚把
            // best_streak 推过 7 的倍数，那一块要在同一次启动里发出
            if let Err(e) = homestead::grant_on_startup(&database) {
                log::warn!("家园方块补发失败: {e}");
            }
            if let Err(e) = season::settle_on_startup(&database) {
                log::warn!("赛季结算失败: {e}");
            }
            // 周报配置自检。放在 manage 之前——database 随后就被移走了
            match app_handle.path().app_data_dir() {
                Ok(dir) => report::startup_check(&database, &dir),
                Err(e) => log::warn!("周报无法定位数据目录: {e}"),
            }
            app.manage(database);

            tray::build(app_handle)?;
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
            // 摸底分级
            placement::get_placement_question,
            placement::submit_placement_answer,
            placement::finalize_placement,
            // 平台能力
            platform::get_user_busy_state,
            // 抽卡与图鉴
            cards::draw_card,
            cards::get_collection,
            cards::mark_cards_seen,
            // 家园建造
            homestead::get_homestead,
            homestead::place_block,
            homestead::remove_block,
            homestead::grant_pending_blocks,
            homestead::get_residents,
            homestead::move_in_resident,
            homestead::move_out_resident,
            homestead::get_blueprints,
            // 赛季赛道
            season::get_season,
            season::redeem_points,
            // 魔王讨伐
            boss::get_boss_words,
            boss::defeat_boss,
            // 统计
            commands::stats::get_today_stats,
            commands::stats::get_overall_stats,
            commands::stats::get_mastery_distribution,
            commands::stats::get_heatmap,
            commands::stats::export_data_json,
            commands::zones::get_zone_progress,
            // 设置
            commands::config::get_setting,
            commands::config::set_setting,
            // 平台能力
            tts::play_word_audio,
            scheduler::get_next_session_time,
            scheduler::trigger_popup_now,
            scheduler::peek_popup_session,
            scheduler::accept_popup,
            scheduler::snooze_popup,
            commands::config::set_autostart,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
