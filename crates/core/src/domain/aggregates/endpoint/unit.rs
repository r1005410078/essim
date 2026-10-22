//! 端点聚合内的逻辑单元。
use crate::domain::{identity::*, model::ModelRef};
use serde::{Deserialize, Serialize};
/// 端点聚合内的逻辑单元；无独立通信角色。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnitConfig {
    /// 单元稳定身份。
    pub id: UnitId,
    /// 显示名称。
    pub name: String,
    /// 展示备注，不参与协议映射。
    #[serde(default)]
    pub note: String,
    /// 线上站号；本阶段产品限制1–247，不是TCP标准上限。
    pub station: u8,
    /// 是否参与下一次应用。
    pub enabled: bool,
    /// 模型身份。
    pub model: ModelRef,
    /// 单层点表配置覆盖；旧配置缺省为完全继承。
    #[serde(default)]
    pub overrides: crate::domain::model::UnitOverrides,
    /// 可选设备关联。
    pub device: Option<DeviceId>,
}
