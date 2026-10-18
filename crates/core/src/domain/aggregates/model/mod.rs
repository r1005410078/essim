//! 采集模型聚合及所属点定义。
/// 聚合根与比较规则。
pub mod aggregate;
/// 聚合内点定义。
pub mod point;
/// 稳定身份引用。
pub mod reference;
pub use aggregate::{protocol_model_matches, CollectionModel};
pub use point::{Area, PointDefinition, Telemetry};
pub use reference::ModelRef;
