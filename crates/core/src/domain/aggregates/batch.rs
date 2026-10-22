//! 批量生命周期任务聚合；未确认结果仍占执行槽，不依赖页面或协议适配器。
use crate::domain::{error::Error, identity::EndpointId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
/// 批量生命周期操作；建立任务不等于操作已执行。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BatchOperation {
    /// 应用已保存配置，完成后停止。
    Apply,
    /// 启动已应用从站或连接已应用主站。
    Start,
    /// 确认资源释放后停止。
    Stop,
}
/// 条目互斥状态；Uncertain表示实际执行尚未结束。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemState {
    /// 尚未领取。
    Pending,
    /// 已开始，保留执行槽。
    Running,
    /// 等待超时，仍保留执行槽。
    Uncertain,
    /// 实际完成成功。
    Succeeded,
    /// 实际完成失败。
    Failed,
    /// 已核对无需执行。
    Skipped,
    /// 取消时尚未开始。
    Cancelled,
}
/// 真实结束后由适配层交付的结果；不含等待超时。
pub enum Outcome {
    /// 通道丢失等情况无法证明实际结束；执行器保留槽位与操作门。
    Unconfirmed(String),
    /// 已确认完成。
    Succeeded,
    /// 已确认失败及原因。
    Failed(String),
    /// 跳过及原因。
    Skipped(String),
}
/// 一项固定目标及其执行计时。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchItem {
    /// 稳定端点身份。
    pub endpoint: EndpointId,
    /// 互斥状态。
    pub state: ItemState,
    /// 当前结果说明。
    pub reason: Option<String>,
    queued_ms: u64,
    started_ms: Option<u64>,
    finished_ms: Option<u64>,
}
impl BatchItem {
    /// 已开始项的排队耗时，单位毫秒。
    pub fn wait_ms(&self) -> Option<u64> {
        self.started_ms.map(|s| s - self.queued_ms)
    }
    /// 执行耗时，结束后固定；运行中按传入的单调时刻计算。
    pub fn execution_ms(&self, now_ms: u64) -> Option<u64> {
        self.started_ms
            .map(|s| self.finished_ms.unwrap_or(now_ms).saturating_sub(s))
    }
}
/// 可核对的互斥状态计数。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    /// 等待数。
    pub pending: usize,
    /// 执行数。
    pub running: usize,
    /// 未确认数。
    pub uncertain: usize,
    /// 成功数。
    pub succeeded: usize,
    /// 失败数。
    pub failed: usize,
    /// 跳过数。
    pub skipped: usize,
    /// 未执行取消数。
    pub cancelled: usize,
}
impl Counts {
    /// 全部条目数，包含未确认项。
    pub fn total(&self) -> usize {
        self.pending
            + self.running
            + self.uncertain
            + self.succeeded
            + self.failed
            + self.skipped
            + self.cancelled
    }
}
/// 单批任务的状态所有者；外部同步保护后才能跨线程使用。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchTask {
    /// 会话任务身份，不是配置版本。
    pub id: String,
    /// 本次固定生命周期操作。
    pub operation: BatchOperation,
    concurrency: usize,
    cancelled: bool,
    last_ms: u64,
    items: Vec<BatchItem>,
}
fn invalid(message: &str) -> Error {
    Error::new("INVALID_BATCH_STATE", message)
}
impl BatchTask {
    /// 固定目标和并发；时间由适配层提供相对单调毫秒。
    /// # Errors
    /// 空目标、重复身份、超过1024目标、并发不在1～8或空任务身份。
    pub fn new(
        id: String,
        operation: BatchOperation,
        targets: Vec<EndpointId>,
        concurrency: usize,
        now_ms: u64,
    ) -> Result<Self, Error> {
        if id.trim().is_empty()
            || targets.is_empty()
            || targets.len() > 1024
            || !(1..=8).contains(&concurrency)
            || targets.iter().any(|t| t.0.trim().is_empty())
            || targets.iter().collect::<BTreeSet<_>>().len() != targets.len()
        {
            return Err(invalid("任务身份、目标或并发不合法"));
        }
        Ok(Self {
            id,
            operation,
            concurrency,
            cancelled: false,
            last_ms: now_ms,
            items: targets
                .into_iter()
                .map(|endpoint| BatchItem {
                    endpoint,
                    state: ItemState::Pending,
                    reason: None,
                    queued_ms: now_ms,
                    started_ms: None,
                    finished_ms: None,
                })
                .collect(),
        })
    }
    /// 只读固定目标及结果。
    pub fn items(&self) -> &[BatchItem] {
        &self.items
    }
    /// 占用执行名额数；未确认项不释放。
    pub fn occupied(&self) -> usize {
        self.items
            .iter()
            .filter(|i| matches!(i.state, ItemState::Running | ItemState::Uncertain))
            .count()
    }
    /// 是否已没有待处理或未完成项。
    pub fn is_finished(&self) -> bool {
        !self.items.iter().any(|i| {
            matches!(
                i.state,
                ItemState::Pending | ItemState::Running | ItemState::Uncertain
            )
        })
    }
    fn check_time(&self, now_ms: u64) -> Result<(), Error> {
        if now_ms < self.last_ms {
            Err(invalid("单调时刻不能倒退"))
        } else {
            Ok(())
        }
    }
    /// 领取下一个目标；取消或执行槽已满时返回None。
    /// # Errors
    /// 单调时刻倒退。
    pub fn claim(&mut self, now_ms: u64) -> Result<Option<EndpointId>, Error> {
        self.check_time(now_ms)?;
        if self.cancelled || self.occupied() >= self.concurrency {
            return Ok(None);
        }
        let Some(item) = self
            .items
            .iter_mut()
            .find(|i| i.state == ItemState::Pending)
        else {
            return Ok(None);
        };
        item.state = ItemState::Running;
        item.started_ms = Some(now_ms);
        self.last_ms = now_ms;
        Ok(Some(item.endpoint.clone()))
    }
    /// 等待超时仅改变观察状态；真实操作继续占用名额。
    /// # Errors
    /// 目标不存在、未执行／已结束或时刻倒退。
    pub fn mark_uncertain(
        &mut self,
        endpoint: &EndpointId,
        now_ms: u64,
        reason: String,
    ) -> Result<(), Error> {
        self.check_time(now_ms)?;
        let item = self
            .items
            .iter_mut()
            .find(|i| &i.endpoint == endpoint)
            .ok_or_else(|| invalid("目标不存在"))?;
        if !matches!(item.state, ItemState::Running | ItemState::Uncertain) {
            return Err(invalid("仅执行中条目可标记未确认"));
        }
        item.state = ItemState::Uncertain;
        item.reason = Some(reason);
        self.last_ms = now_ms;
        Ok(())
    }
    /// 实际执行结束后结算；成功项不会因其他项失败而回滚。
    /// # Errors
    /// 目标不存在、未执行／重复结束或时刻倒退。
    pub fn complete(
        &mut self,
        endpoint: &EndpointId,
        now_ms: u64,
        outcome: Outcome,
    ) -> Result<(), Error> {
        self.check_time(now_ms)?;
        let item = self
            .items
            .iter_mut()
            .find(|i| &i.endpoint == endpoint)
            .ok_or_else(|| invalid("目标不存在"))?;
        if !matches!(item.state, ItemState::Running | ItemState::Uncertain) {
            return Err(invalid("仅占槽条目可以结算"));
        }
        let (state, reason) = match outcome {
            Outcome::Unconfirmed(_) => return Err(invalid("未确认结果不能作为实际完成提交")),
            Outcome::Succeeded => (ItemState::Succeeded, None),
            Outcome::Failed(r) => (ItemState::Failed, Some(r)),
            Outcome::Skipped(r) => (ItemState::Skipped, Some(r)),
        };
        item.state = state;
        item.reason = reason;
        item.finished_ms = Some(now_ms);
        self.last_ms = now_ms;
        Ok(())
    }
    /// 取消后不领取新项；执行中与未确认项保持状态直到真实结束。
    /// # Errors
    /// 单调时刻倒退。
    pub fn cancel(&mut self, now_ms: u64) -> Result<(), Error> {
        self.check_time(now_ms)?;
        self.cancelled = true;
        for item in &mut self.items {
            if item.state == ItemState::Pending {
                item.state = ItemState::Cancelled;
                item.reason = Some("任务取消，未执行".into());
                item.finished_ms = Some(now_ms);
            }
        }
        self.last_ms = now_ms;
        Ok(())
    }
    /// 按互斥状态聚合，不将未确认项重复计入执行中。
    pub fn counts(&self) -> Counts {
        let mut c = Counts::default();
        for item in &self.items {
            match item.state {
                ItemState::Pending => c.pending += 1,
                ItemState::Running => c.running += 1,
                ItemState::Uncertain => c.uncertain += 1,
                ItemState::Succeeded => c.succeeded += 1,
                ItemState::Failed => c.failed += 1,
                ItemState::Skipped => c.skipped += 1,
                ItemState::Cancelled => c.cancelled += 1,
            }
        }
        c
    }
}
