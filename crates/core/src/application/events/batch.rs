//! 批量任务会话事实；不代表配置提交或协议成功确认。
use crate::{application::queries::batch::BatchView, domain::identity::EndpointId};
use serde::Serialize;
/// 已发生的任务状态变化。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BatchFact {
    /// 清单已固定。
    Started,
    /// 一个目标已取得执行槽。
    ItemStarted {
        /// 端点身份。
        endpoint: EndpointId,
    },
    /// 后续项已取消，已开始项继续。
    Cancelled,
    /// 等待结果超时，未释放槽。
    ItemUncertain {
        /// 端点身份。
        endpoint: EndpointId,
    },
    /// 真实操作已结束；结果见投影。
    ItemCompleted {
        /// 端点身份。
        endpoint: EndpointId,
    },
}
/// 状态事实与有界投影一起交付，序号仅为通知去重。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchEvent {
    /// 会话内通知序号。
    pub sequence: u64,
    /// 实际状态变化。
    pub fact: BatchFact,
    /// 状态变化后的投影。
    pub view: BatchView,
}
