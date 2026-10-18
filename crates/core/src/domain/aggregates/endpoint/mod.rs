//! 通信端点聚合及所属单元和值对象。
/// 聚合根。
pub mod aggregate;
/// 通信值对象。
pub mod transport;
/// 聚合内单元。
pub mod unit;
pub use aggregate::EndpointConfig;
pub use transport::{Parity, Role, Transport};
pub use unit::UnitConfig;
