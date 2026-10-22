//! 批量任务摘要查询；没有配置模型或运行点值。
use crate::domain::{
    aggregates::batch::{BatchOperation, BatchTask, Counts, ItemState},
    identity::EndpointId,
};
use serde::Serialize;
/// 一项任务结果与相对耗时。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchItemView {
    /// 固定目标身份。
    pub endpoint: EndpointId,
    /// 当前状态。
    pub state: ItemState,
    /// 跳过、失败或未确认原因。
    pub reason: Option<String>,
    /// 已开始项排队毫秒。
    pub wait_ms: Option<u64>,
    /// 执行毫秒；未确认仍持续计时。
    pub execution_ms: Option<u64>,
}
/// 有界任务投影，最多1024项。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchView {
    /// 当前任务身份。
    pub id: String,
    /// 本次生命周期操作。
    pub operation: BatchOperation,
    /// 真实完成，不把超时当作结束。
    pub finished: bool,
    /// 实际占用槽，包括未确认项。
    pub occupied: usize,
    /// 互斥计数。
    pub counts: Counts,
    /// 固定清单及逐项结果。
    pub items: Vec<BatchItemView>,
}
impl BatchView {
    /// 从聚合构造轻量摘要；now_ms来自同一单调时间基准。
    pub fn from_task(task: &BatchTask, now_ms: u64) -> Self {
        Self {
            id: task.id.clone(),
            operation: task.operation,
            finished: task.is_finished(),
            occupied: task.occupied(),
            counts: task.counts(),
            items: task
                .items()
                .iter()
                .map(|i| BatchItemView {
                    endpoint: i.endpoint.clone(),
                    state: i.state,
                    reason: i.reason.clone(),
                    wait_ms: i.wait_ms(),
                    execution_ms: i.execution_ms(now_ms),
                })
                .collect(),
        }
    }
}
