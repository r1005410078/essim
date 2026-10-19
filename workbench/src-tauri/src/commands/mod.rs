//! Tauri接口层：DTO、配置命令／查询及订阅分别组织。
mod dto;
pub mod point_table;
pub mod simulator;
pub mod subscriptions;
use essim_infrastructure::workspace::WorkspaceEngine;
use std::sync::Arc;
/// 注入到所有命令的同一应用服务；不依赖具体窗口，也不伪造协议执行器。
pub type Engine = Arc<WorkspaceEngine>;

/// 真实TCP从站运行命令与按需查询。
pub mod slave;

/// TCP主站IPC。
pub mod master;
