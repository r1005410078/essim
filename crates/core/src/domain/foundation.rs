//! 旧领域导入门面，仅重导出；新代码使用对应领域模块。
pub use super::point_table::{Order, RawValue, ValueType};
pub use super::serialization::decimal;
pub use super::{
    device::*,
    endpoint::{EndpointConfig, Parity, Role, Transport, UnitConfig},
    error::*,
    identity::*,
    model::{protocol_model_matches, Area, CollectionModel, ModelRef, PointDefinition, Telemetry},
    navigation::*,
    values::*,
};
