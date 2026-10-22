//! 批量目标规划的有界摘要；不序列化模型、运行值或内部比较快照。
use crate::domain::{aggregates::batch::BatchOperation, endpoint::Role, identity::EndpointId};
use serde::Serialize;
/// 一项固定的端点摘要。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchTargetSummary {
    /// 稳定目标身份。
    pub endpoint: EndpointId,
    /// 预览时的名称。
    pub name: String,
    /// 通信角色。
    pub role: Role,
    /// 单元数量，不含完整定义。
    pub units: usize,
    /// 是否已有应用快照；不是运行状态。
    pub applied: bool,
    /// 本项沿用单端点操作的观察等待毫秒，超时不代表实际结束。
    pub wait_ms: u64,
}
/// 一次预览固定的范围及操作。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchPlanPreview {
    /// 会话任务身份，不是配置版本。
    pub id: String,
    /// 固定的生命周期操作。
    pub operation: BatchOperation,
    /// 本次并发上限。
    pub concurrency: usize,
    /// 去重且固定的端点清单。
    pub targets: Vec<BatchTargetSummary>,
}
