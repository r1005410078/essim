//! 可选设备实体，独立于通信端点及角色。
use crate::domain::identity::DeviceId;
use serde::{Deserialize, Serialize};
/// 可选关联的设备实体；并非必须存在的真实硬件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    /// 设备身份。
    pub id: DeviceId,
    /// 名称。
    pub name: String,
    /// 自定义设备类型，例如PCS。
    pub kind: String,
}
