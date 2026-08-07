//! Tauri command 层。
//!
//! 职责边界：校验前端输入、取数据库锁、委托给 repo。业务逻辑不写在这里——
//! 排队在 `queue`，作答落库在 `review`，FSRS 计算在前端（ADR-2）。

pub mod config;
pub mod library;
pub mod session;
pub mod stats;
pub mod zones;
