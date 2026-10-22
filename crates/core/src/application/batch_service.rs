//! 批量任务用例：命令改变聚合，具体事实更新查询投影，不执行协议IO。
use crate::{
    application::{
        commands::batch::BatchCommand,
        event_handlers::batch_projection::BatchProjectionListener,
        events::batch::{BatchEvent, BatchFact},
        queries::batch::BatchView,
    },
    domain::{aggregates::batch::BatchTask, error::Error},
};
/// 单活动任务用例；外部固定执行器负责真实操作及同步保护。
#[derive(Default)]
pub struct BatchService {
    task: Option<BatchTask>,
    sequence: u64,
    projection: BatchProjectionListener,
}
impl BatchService {
    /// 处理内部任务命令，只有发生状态变化才返回具体事实。
    /// # Errors
    /// 活动任务占用、身份不匹配、状态转换非法、时刻倒退或通知序号耗尽。
    pub fn execute(
        &mut self,
        command: BatchCommand,
        now_ms: u64,
    ) -> Result<Option<BatchEvent>, Error> {
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| Error::new("SEQUENCE_OVERFLOW", "批量通知序号耗尽"))?;
        let fact = match command {
            BatchCommand::Start {
                id,
                operation,
                targets,
                concurrency,
            } => {
                if self.task.as_ref().is_some_and(|t| !t.is_finished()) {
                    return Err(Error::new(
                        "BATCH_BUSY",
                        "已有批量任务，未确认项结束前不能开始新任务",
                    ));
                }
                self.task = Some(BatchTask::new(id, operation, targets, concurrency, now_ms)?);
                BatchFact::Started
            }
            other => {
                let task = self
                    .task
                    .as_mut()
                    .ok_or_else(|| Error::new("BATCH_NOT_FOUND", "批量任务不存在"))?;
                let id = match &other {
                    BatchCommand::Claim { id }
                    | BatchCommand::Cancel { id }
                    | BatchCommand::MarkUncertain { id, .. }
                    | BatchCommand::Complete { id, .. } => id,
                    BatchCommand::Start { .. } => {
                        return Err(Error::new("INVALID_BATCH_STATE", "内部命令分派不一致"));
                    }
                };
                if &task.id != id {
                    return Err(Error::new("BATCH_NOT_FOUND", "批量任务身份不匹配"));
                }
                match other {
                    BatchCommand::Claim { .. } => {
                        let Some(endpoint) = task.claim(now_ms)? else {
                            return Ok(None);
                        };
                        BatchFact::ItemStarted { endpoint }
                    }
                    BatchCommand::Cancel { .. } => {
                        task.cancel(now_ms)?;
                        BatchFact::Cancelled
                    }
                    BatchCommand::MarkUncertain {
                        endpoint, reason, ..
                    } => {
                        task.mark_uncertain(&endpoint, now_ms, reason)?;
                        BatchFact::ItemUncertain { endpoint }
                    }
                    BatchCommand::Complete {
                        endpoint, outcome, ..
                    } => {
                        task.complete(&endpoint, now_ms, outcome)?;
                        BatchFact::ItemCompleted { endpoint }
                    }
                    BatchCommand::Start { .. } => {
                        return Err(Error::new("INVALID_BATCH_STATE", "内部命令分派不一致"));
                    }
                }
            }
        };
        let view = BatchView::from_task(
            self.task
                .as_ref()
                .ok_or_else(|| Error::new("INVALID_BATCH_STATE", "任务状态缺失"))?,
            now_ms,
        );
        let event = BatchEvent {
            sequence,
            fact,
            view,
        };
        self.projection.handle(&event)?;
        self.sequence = sequence;
        Ok(Some(event))
    }
    /// 只复制最多1024项摘要；耗时按同一单调基准采样，不复制模型或点值。
    pub fn query(&self, now_ms: u64) -> Option<BatchView> {
        self.task.as_ref().map(|t| BatchView::from_task(t, now_ms))
    }
    /// 最近事实的监听投影，便于订阅恢复后检查状态。
    pub fn projection(&self) -> Option<&BatchView> {
        self.projection.query()
    }
}
