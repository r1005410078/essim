//! 显式同步事件消费者；不提供事件总线、动态注册或可靠投递。
/// 工作区查询投影监听器。
pub mod workspace_projection;
pub use workspace_projection::WorkspaceProjectionListener;

/// 批量任务轻量投影监听。
pub mod batch_projection;
