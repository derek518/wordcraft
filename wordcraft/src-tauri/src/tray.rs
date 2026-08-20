//! 系统托盘。spec F7。
//!
//! 应用常驻后台等待时段弹窗，关闭窗口只是隐藏——托盘是用户唯一能再次
//! 找到它、以及主动退出的入口。没有托盘，「关掉了但还在跑」会让人以为
//! 程序失控。

use crate::db::{clock, repo::sessions, repo::settings, Db};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Runtime,
};
use tauri_plugin_autostart::ManagerExt;

pub fn build<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let train = MenuItem::with_id(app, "train", "立即训练", true, None::<&str>)
        .map_err(|e| format!("创建菜单项失败: {e}"))?;
    let pause = MenuItem::with_id(app, "pause", "今日暂停", true, None::<&str>)
        .map_err(|e| format!("创建菜单项失败: {e}"))?;
    let sep = PredefinedMenuItem::separator(app).map_err(|e| format!("创建分隔符失败: {e}"))?;
    let autostart = MenuItem::with_id(app, "autostart", "开机自启", true, None::<&str>)
        .map_err(|e| format!("创建菜单项失败: {e}"))?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)
        .map_err(|e| format!("创建菜单项失败: {e}"))?;

    let menu = Menu::with_items(app, &[&train, &pause, &sep, &autostart, &quit])
        .map_err(|e| format!("创建托盘菜单失败: {e}"))?;

    TrayIconBuilder::with_id("main")
        .icon(
            app.default_window_icon()
                .ok_or("应用缺少默认图标")?
                .clone(),
        )
        .tooltip("WordCraft 词匠")
        .menu(&menu)
        // 左键点击图标直接显示窗口，不必每次都展开菜单
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| match event.id.as_ref() {
            "train" => show_main_window(app),
            "pause" => {
                if let Err(e) = activate_pause(app) {
                    log::warn!("激活今日暂停失败: {e}");
                }
            }
            "autostart" => {
                match app.autolaunch().is_enabled() {
                    Ok(currently) => {
                        if let Err(e) = crate::commands::config::apply_autostart(app, !currently) {
                            log::warn!("切换开机自启失败: {e}");
                        }
                    }
                    Err(e) => log::warn!("读取自启状态失败: {e}"),
                }
            }
            "quit" => app.exit(0),
            other => log::warn!("未处理的托盘菜单项: {other}"),
        })
        .build(app)
        .map_err(|e| format!("创建托盘图标失败: {e}"))?;

    Ok(())
}

fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    let Some(win) = app.get_webview_window("main") else {
        log::warn!("主窗口不存在，无法显示");
        return;
    };
    let _ = win.show();
    let _ = win.unminimize();
    let _ = win.set_focus();
}

fn activate_pause<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    use crate::db::repo::player_stats;

    const MONTHLY_PAUSE_QUOTA: i64 = 2;

    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|e| format!("获取数据库锁失败: {e}"))?;
    let today = clock::today();

    if sessions::daily_record(&conn, &today)?.is_paused {
        return Err("今日已处于暂停状态".to_string());
    }

    let remaining = player_stats::use_pause_quota(&conn, MONTHLY_PAUSE_QUOTA)?;
    sessions::set_paused(&conn, &today, true)?;
    settings::set(&conn, "daily_pause_date", &today)?;

    log::info!("今日暂停已激活，本月剩余 {remaining} 次");
    Ok(())
}
