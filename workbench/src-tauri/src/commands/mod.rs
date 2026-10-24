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

/// 串口发现IPC。
pub mod serial;

/// 完整点位分组配置IPC。
pub mod partition;

/// 固定批量会话接口。
pub mod batch;

/// 工程文件与本机恢复状态。
pub mod project;

mod project_access;
