//! Windows 平台能力。contracts §3.5。
//!
//! **此文件无法在开发机（macOS）上编译与验证**，只在 Windows 构建时参与编译。
//! 首次在 Windows 上运行时必须实测：打开全屏游戏，确认 `user_busy_state`
//! 返回 `FullScreenD3D` 而非 `Normal`。

use super::{BusyState, PlatformError, PlatformIntegration};
use windows::Win32::UI::Shell::{
    SHQueryUserNotificationState, QUNS_ACCEPTS_NOTIFICATIONS, QUNS_APP, QUNS_BUSY,
    QUNS_NOT_PRESENT, QUNS_PRESENTATION_MODE, QUNS_QUIET_TIME, QUNS_RUNNING_D3D_FULL_SCREEN,
};

pub struct WindowsPlatform;

impl PlatformIntegration for WindowsPlatform {
    fn user_busy_state(&self) -> Result<BusyState, PlatformError> {
        // SAFETY: 无参数调用，仅读取系统状态，不持有任何指针
        let state = unsafe {
            SHQueryUserNotificationState()
                .map_err(|e| PlatformError(format!("SHQueryUserNotificationState 失败: {e}")))?
        };

        Ok(match state {
            QUNS_ACCEPTS_NOTIFICATIONS => BusyState::Normal,
            QUNS_RUNNING_D3D_FULL_SCREEN => BusyState::FullScreenD3D,
            QUNS_BUSY => BusyState::Busy,
            QUNS_PRESENTATION_MODE => BusyState::Presentation,
            // 锁屏、安静时段、全屏非 D3D 应用都归为「不宜打扰」而非 Normal。
            // 归错方向的代价不对称：误判空闲会在用户看电影时弹窗，
            // 误判忙碌只是少弹一次
            QUNS_NOT_PRESENT | QUNS_QUIET_TIME | QUNS_APP => BusyState::Busy,
            other => {
                log::warn!("未知的用户通知状态 {other:?}，按未知处理");
                BusyState::Unknown
            }
        })
    }
}
