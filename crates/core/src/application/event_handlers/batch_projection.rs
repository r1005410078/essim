//! 同步消费批量事实更新轻量查询投影；不读完整工作区。
use crate::{
    application::{events::batch::BatchEvent, queries::batch::BatchView},
    domain::error::Error,
};
/// 会话内最近任务投影，旧通知不得倒退状态。
#[derive(Default)]
pub struct BatchProjectionListener {
    sequence: u64,
    view: Option<BatchView>,
}
impl BatchProjectionListener {
    /// 消费已发生的任务事实；失败保持原投影。
    /// # Errors
    /// 通知序号倒退／重复、摘要计数不一致或任务身份为空。
    pub fn handle(&mut self, event: &BatchEvent) -> Result<(), Error> {
        let view = &event.view;
        if event.sequence <= self.sequence
            || view.id.trim().is_empty()
            || view.counts.total() != view.items.len()
            || view.occupied != view.counts.running + view.counts.uncertain
            || view.finished != (view.counts.pending == 0 && view.occupied == 0)
        {
            return Err(Error::new(
                "INVALID_BATCH_EVENT",
                "批量通知失序或状态不一致",
            ));
        }
        self.sequence = event.sequence;
        self.view = Some(view.clone());
        Ok(())
    }
    /// 最近事实对应的轻量投影；执行耗时为该事实时刻采样。
    pub fn query(&self) -> Option<&BatchView> {
        self.view.as_ref()
    }
}
