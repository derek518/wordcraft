//! 平台能力抽象。ADR-3、contracts §3.5。
//!
//! 目标平台是 Windows，开发机是 macOS。这层抽象的价值不在「支持多平台」，
//! 而在**让能力缺失无法伪装成一切正常**——非 Windows 实现返回 `Unknown`
//! 并记 warn，调用方被迫显式处理，而不是收到一个看起来正常的 `Normal`。
//!
//! 那正是审计 D6 那类 silent fallback 的反面。

use serde::Serialize;

#[cfg(target_os = "windows")]
mod windows_impl;

#[cfg(not(target_os = "windows"))]
mod stub;

/// 用户当前是否适合被打扰。对应 Windows 的 `QUERY_USER_NOTIFICATION_STATE`。
///
/// 非 Windows 构建下只有 `Unknown` 会被构造，其余变体由 `windows_impl` 产出，
/// 而那个文件不参与非 Windows 编译——所以此处按平台放行 dead_code，
/// 而不是无条件 allow（Windows 上仍应对未使用的变体报警）。
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BusyState {
    /// 可以正常打扰
    Normal,
    /// 全屏 D3D 应用（游戏）
    FullScreenD3D,
    /// 用户处于忙碌状态
    Busy,
    /// 演示模式
    Presentation,
    /// **查不到**。不是「正常」的同义词——调用方必须能区分
    /// 「确认用户空闲」与「无从得知」
    Unknown,
}

impl BusyState {
    /// 是否应当避免弹窗。
    ///
    /// `Unknown` 归为「可打扰」是有意的：开发机永远拿不到真实状态，
    /// 若在此保守跳过，弹窗功能在开发期完全无法验证。Windows 上有真实
    /// 检测，这个分支不会生效。
    pub fn should_suppress(self) -> bool {
        matches!(self, Self::FullScreenD3D | Self::Busy | Self::Presentation)
    }
}

#[derive(Debug)]
pub struct PlatformError(pub String);

impl std::fmt::Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub trait PlatformIntegration: Send + Sync {
    fn user_busy_state(&self) -> Result<BusyState, PlatformError>;
}

#[cfg(target_os = "windows")]
pub fn integration() -> Box<dyn PlatformIntegration> {
    Box::new(windows_impl::WindowsPlatform)
}

#[cfg(not(target_os = "windows"))]
pub fn integration() -> Box<dyn PlatformIntegration> {
    Box::new(stub::StubPlatform)
}

/// contracts §3.5：查询用户忙碌状态。
#[tauri::command]
pub fn get_user_busy_state() -> Result<BusyState, String> {
    integration()
        .user_busy_state()
        .map_err(|e| format!("查询用户状态失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 未知状态不抑制弹窗() {
        // 开发机永远返回 Unknown。若把它当作「可能在忙」而跳过弹窗，
        // 整个调度功能在开发期无法验证
        assert!(!BusyState::Unknown.should_suppress());
        assert!(!BusyState::Normal.should_suppress());
    }

    #[test]
    fn 全屏与忙碌状态抑制弹窗() {
        assert!(BusyState::FullScreenD3D.should_suppress());
        assert!(BusyState::Busy.should_suppress());
        assert!(BusyState::Presentation.should_suppress());
    }

    #[test]
    fn 非_windows_平台返回未知而非伪装正常() {
        // 契约明确要求：stub 必须返回 Unknown 并记 warn，
        // 返回 Normal 会让「没有检测能力」看起来像「确认用户空闲」
        let state = integration().user_busy_state().unwrap();

        #[cfg(not(target_os = "windows"))]
        assert_eq!(
            state,
            BusyState::Unknown,
            "非 Windows 平台必须返回 Unknown，不得伪装成 Normal"
        );

        #[cfg(target_os = "windows")]
        assert_ne!(state, BusyState::Unknown, "Windows 上应能查到真实状态");
    }
}
