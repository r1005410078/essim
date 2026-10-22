//! 批量任务内部命令；条目完成命令只能由真实执行适配器交付，不是前端成功按钮。
use crate::domain::{
    aggregates::batch::{BatchOperation, Outcome},
    identity::EndpointId,
};
/// 会话批量任务命令；任务身份不属于配置版本。
pub enum BatchCommand {
    /// 固定目标并建立任务，尚不隐式应用／启动。
    Start {
        /// 任务身份。
        id: String,
        /// 已确认的生命周期操作。
        operation: BatchOperation,
        /// 已解析且固定的目标。
        targets: Vec<EndpointId>,
        /// 并发1～8。
        concurrency: usize,
    },
    /// 领取一个执行槽。
    Claim {
        /// 任务身份。
        id: String,
    },
    /// 取消尚未开始项。
    Cancel {
        /// 任务身份。
        id: String,
    },
    /// 调用等待超时，后台尚未结束。
    MarkUncertain {
        /// 任务身份。
        id: String,
        /// 端点身份。
        endpoint: EndpointId,
        /// 未确认原因。
        reason: String,
    },
    /// 后台操作真实结束。
    Complete {
        /// 任务身份。
        id: String,
        /// 端点身份。
        endpoint: EndpointId,
        /// 实际完成结果。
        outcome: Outcome,
    },
}
