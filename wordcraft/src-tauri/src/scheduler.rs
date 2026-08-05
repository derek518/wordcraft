use std::thread;
use std::time::Duration;
use tauri::AppHandle;
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct SessionTime {
    pub next_session: String,
    pub minutes_until: i64,
    pub session_type: String,
}

static mut SCHEDULER_HANDLE: Option<thread::JoinHandle<()>> = None;

pub fn start_scheduler(_app: AppHandle) {
    // Scheduler thread - simplified for MVP
    // In production, would check system time and show popups
    let handle = thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(60));
        }
    });
    
    unsafe {
        SCHEDULER_HANDLE = Some(handle);
    }
}

#[tauri::command]
pub fn get_next_session_time() -> SessionTime {
    // Simplified - always return next morning session
    SessionTime {
        next_session: "09:00".to_string(),
        minutes_until: 60,
        session_type: "morning".to_string(),
    }
}

#[tauri::command]
pub fn trigger_popup_now(_app: AppHandle) -> Result<(), String> {
    Ok(())
}
