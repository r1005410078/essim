//! 主站轻量查询与按点读取质量。
use super::slave::SlaveObservation;
use crate::domain::{
    identity::{EndpointId, PointId, UnitId},
    point_table::RawValue,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
/// 按需查询，不等待协议网络。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MasterQuery {
    /// 端点。
    pub endpoint: EndpointId,
    /// 单元。
    pub unit: Option<UnitId>,
    /// 最多1024个点。
    #[serde(default)]
    pub points: Vec<PointId>,
}
/// 最近成功值与质量独立，失败不伪造零值。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MasterPoint {
    /// 最近成功原始值。
    pub value: Option<RawValue>,
    /// 最近成功Unix毫秒。
    pub unix_ms: Option<String>,
    /// unread、valid、stale、failed、disconnected。
    pub quality: String,
    /// 具体读取故障。
    pub error: Option<String>,
    /// 本批写入结果；不等于设备动作完成。
    pub write_result: Option<String>,
    /// 下发结果所属的命令身份，避免旧成功结果确认新草稿。
    pub write_operation: Option<String>,
}
/// 主站独立投影，不复制全量模型。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MasterView {
    /// 目标端点。
    pub endpoint: EndpointId,
    /// disconnected、connecting、connected、reconnecting、failed。
    pub state: String,
    /// 周期采集意图，断线重连时仍为true。
    pub polling: bool,
    /// 当前暂停周期采集的目标单元。
    pub paused_units: Vec<UnitId>,
    /// 手动批次或连接操作正在执行。
    pub busy: bool,
    /// 当前操作身份。
    pub operation: Option<String>,
    /// 已完成块数。
    pub completed: usize,
    /// 总块数。
    pub total: usize,
    /// 实际请求数。
    pub requests: u64,
    /// 最近故障。
    pub error: Option<String>,
    /// 投影序号。
    pub revision: String,
    /// 按需点质量和值。
    pub points: BTreeMap<PointId, MasterPoint>,
    /// 最近最多100条实际观测；不是完整历史。
    pub observations: Vec<SlaveObservation>,
    /// 完整可靠记录文件。
    pub record_path: Option<String>,
}
