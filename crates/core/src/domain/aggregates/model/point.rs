//! 采集模型所属点定义及分类。
use crate::domain::{
    identity::PointId,
    point_table::{Order, RawValue, ValueType},
};
use serde::{Deserialize, Serialize};
/// 四遥业务分类，不决定 Modbus 数据区。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Telemetry {
    /// 遥测。
    Measurement,
    /// 遥信。
    Indication,
    /// 遥控。
    Control,
    /// 遥调。
    Adjustment,
}
/// Modbus 四种独立数据区；地址均为零起始。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Area {
    /// 保持寄存器。
    Holding,
    /// 输入寄存器。
    Input,
    /// 线圈。
    Coil,
    /// 离散输入。
    Discrete,
}
/// 点定义；公共映射只定义一次，角色字段并存。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PointDefinition {
    /// 稳定点身份。
    pub id: PointId,
    /// 点名。
    pub name: String,
    /// 四遥分类。
    pub category: Telemetry,
    /// 协议数据区。
    pub area: Area,
    /// 零起始地址，非4xxxx展示编号。
    pub address: u16,
    /// 原始值类型。
    pub value_type: ValueType,
    /// 原始值乘此精确十进制倍率得到工程值。
    pub scale: String,
    /// 工程单位。
    pub unit: String,
    /// 从站初始原始值，由工程输入反算并校验。
    pub initial: RawValue,
    /// 用户输入的初始工程值，保留十进制文本精度。
    pub initial_engineering: String,
    /// 寄存器内部字节顺序。
    pub byte_order: Order,
    /// 多寄存器16位字的顺序。
    pub word_order: Order,
    /// 用户备注。
    pub note: String,
    /// 从站是否允许外部写入。
    pub slave_writable: bool,
    /// 主站建议采集周期，毫秒。
    pub master_period_ms: u32,
}
