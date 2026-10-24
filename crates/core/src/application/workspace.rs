//! 旧配置应用导入门面，仅重导出；实现见commands、queries、events、ports和service。
pub use super::catalog::*;
pub use super::commands::{Action, Command, NewNavigationResource, Receipt};
pub use super::events::{ConfigEvent, ConfigurationFact};
pub use super::ports::*;
pub use super::queries::*;
pub use super::service::*;
pub use crate::domain::navigation::{Folder, Navigation};
