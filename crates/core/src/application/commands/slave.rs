//! TCP从站运行意图，独立于配置提交。
use crate::domain::{
    identity::{EndpointId, PointId, UnitId},
    point_table::RawValue,
};
use serde::{Deserialize, Serialize};
/// 运行命令携带稳定因果身份，不能解释为配置保存。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlaveCommand {
    /// 用户操作因果身份。
    pub command_id: String,
    /// 目标端点。
    pub endpoint: EndpointId,
    /// 运行操作。
    pub action: SlaveAction,
}
/// 由端点所有者串行执行的操作。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SlaveAction {
    /// 使用已应用快照启动真实监听。
    Start {
        /// 同时客户端上限，1～100，默认10。
        #[serde(default = "default_connections", rename = "maxClients")]
        max_clients: usize,
    },
    /// 关闭连接并确认监听资源释放。
    Stop,
    /// 当前Unit内原子置数；工程值须先由既有编码规则转换。
    SetValues {
        /// Unit稳定身份。
        unit: UnitId,
        /// 稳定点身份及精确原始值。
        values: Vec<(PointId, RawValue)>,
    },
    /// 恢复已应用初值。
    ResetValues {
        /// Unit稳定身份。
        unit: UnitId,
        /// 空列表恢复整个Unit，否则只恢复选中点。
        points: Vec<PointId>,
    },
}
fn default_connections() -> usize {
    10
}
