//! TCP从站轻量运行摘要与按需点值查询，不携带完整配置目录。
use crate::domain::{
    identity::{EndpointId, PointId, UnitId},
    point_table::RawValue,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
/// 查询请求，最多查询一个Unit的1024个点。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlaveQuery {
    /// 目标端点。
    pub endpoint: EndpointId,
    /// 需要点值的Unit；只查摘要时为空。
    pub unit: Option<UnitId>,
    /// 按需点身份。
    #[serde(default)]
    pub points: Vec<PointId>,
}
/// 真实状态，失败不等于资源已释放。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SlaveState {
    /// 尚未建立运行。
    Idle,
    /// 启动准备中。
    Starting,
    /// 实际监听中。
    Running,
    /// 正在停止。
    Stopping,
    /// 已确认通信资源释放。
    Stopped,
    /// 本次运行失败，故障见摘要。
    Failed,
}
/// 可靠记录的轻量观测条目，不包含批量置数载荷。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlaveObservation {
    /// 当前运行内的可靠记录序号，精确字符串。
    pub sequence: String,
    /// 与记录文件一致的Unix毫秒时间，精确字符串。
    pub unix_ms: String,
    /// 事件摘要JSON；通信事件保留实际字节。
    pub event: String,
}
/// 独立运行摘要，不使用配置通知序号排序。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlaveView {
    /// 端点身份。
    pub endpoint: EndpointId,
    /// 运行状态。
    pub state: SlaveState,
    /// 独立运行修订号，精确字符串。
    pub revision: String,
    /// 当前连接数。
    pub clients: usize,
    /// 连接上限。
    pub max_clients: usize,
    /// 已处理请求数量。
    pub requests: String,
    /// 协议异常响应数。
    pub exceptions: String,
    /// 最近请求处理耗时，微秒，不含远端确认。
    pub last_request_micros: String,
    /// 具体故障信息。
    pub error: Option<String>,
    /// 记录文件路径（内部诊断用途）。
    pub record_path: Option<String>,
    /// 最后可靠记录位置。
    pub last_reliable: String,
    /// 是否存在结果不确定范围。
    pub uncertain: bool,
    /// 最近至多100条简要事件。
    pub events: Vec<String>,
    /// 最近至多100条带时间与序号的可靠观测。
    pub observations: Vec<SlaveObservation>,
    /// 按请求返回的原始运行值。
    pub values: BTreeMap<PointId, RawValue>,
}
