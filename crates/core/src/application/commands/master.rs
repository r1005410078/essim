//! 主站运行意图，与配置保存分开。
use crate::domain::{
    identity::{EndpointId, PointId, UnitId},
    point_table::RawValue,
};
use serde::{Deserialize, Serialize};
/// 用户操作及因果身份。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MasterCommand {
    /// 操作身份。
    pub command_id: String,
    /// 目标端点。
    pub endpoint: EndpointId,
    /// 运行操作。
    pub action: MasterAction,
}
/// 由所属端点串行处理的主站操作。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MasterAction {
    /// 使用已应用配置连接，不自动采集。
    Connect,
    /// 已连接时开始周期采集。
    StartPolling,
    /// 停止周期采集，保留连接。
    StopPolling,
    /// 暂停或恢复单个目标的周期采集，不关闭共享连接。
    SetUnitPolling {
        /// 目标单元。
        unit: UnitId,
        /// 是否采集。
        enabled: bool,
    },
    /// 取消余项并释放连接。
    Disconnect,
    /// 手动读取完整点；空列表读取该Unit全部点。
    Read {
        /// 单元身份。
        unit: UnitId,
        /// 点身份。
        points: Vec<PointId>,
    },
    /// 整批预校验后逐请求写入并回验。
    Write {
        /// 单元身份。
        unit: UnitId,
        /// 精确原始待写值。
        values: Vec<(PointId, RawValue)>,
    },
}
