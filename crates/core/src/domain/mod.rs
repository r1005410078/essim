//! 领域规则，不依赖窗口、仓储或应用用例。
/// 聚合根及所属实体和值对象。
pub mod aggregates;
/// 可选设备实体。
pub use aggregates::device;
/// 端点聚合及单元规则。
pub use aggregates::endpoint;
/// 结构化业务错误。
pub mod error;
/// 旧导入重导出门面。
pub mod foundation;
/// 稳定业务身份。
pub mod identity;
/// 采集模型及协议定义。
pub use aggregates::model;
/// 独立会话导航规则。
pub use aggregates::navigation;
/// 精确编码、十进制与整表校验。
pub mod point_table;
/// 精确跨边界序列化。
pub mod serialization;
/// 已应用快照与值迁移。
pub mod values;
/// 从站模型索引与纯点值规划。
pub mod slave;

/// TCP主站职责。
pub mod master;
