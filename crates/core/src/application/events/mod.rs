//! 配置提交事实与事件信封；不承诺可靠投递。
/// 提交事件信封。
pub mod config_event;
/// 具体配置业务事实。
pub mod configuration;
pub use config_event::ConfigEvent;
pub use configuration::ConfigurationFact;
/// 运行事实与历史事件契约。
pub mod history;

/// 真实TCP从站运行事实。
pub mod slave;

/// TCP主站职责。
pub mod master;
