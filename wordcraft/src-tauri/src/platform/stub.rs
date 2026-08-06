//! 非 Windows 平台的能力桩。MOCKS.md S1——**唯一允许长期存在的 stub**。
//!
//! 它的正当性在于开发机是 macOS，而 `SHQueryUserNotificationState` 是
//! Windows 专有 API。它之所以安全，是因为返回 `Unknown` 强制调用方显式处理：
//! 能力缺失无法伪装成一切正常。

use super::{BusyState, PlatformError, PlatformIntegration};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct StubPlatform;

/// warn 只记一次——调度器每 30 秒查一次状态，每次都记会把日志淹没，
/// 真正的问题反而找不到。
static WARNED: AtomicBool = AtomicBool::new(false);

impl PlatformIntegration for StubPlatform {
    fn user_busy_state(&self) -> Result<BusyState, PlatformError> {
        if !WARNED.swap(true, Ordering::Relaxed) {
            log::warn!(
                "当前平台无法检测用户忙碌状态（需要 Windows 的 SHQueryUserNotificationState）。\
                 弹窗将不区分用户是否在全屏应用中"
            );
        }
        Ok(BusyState::Unknown)
    }
}
