//! 按业务边界组织聚合及可选设备实体；不把会话Catalog视作单一聚合。
/// 可选设备实体。
pub mod device;
/// 通信端点聚合。
pub mod endpoint;
/// 采集模型聚合。
pub mod model;
/// 独立会话导航。
pub mod navigation;

/// 批量生命周期任务与未确认占位。
pub mod batch;
