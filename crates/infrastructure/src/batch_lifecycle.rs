//! 会话级批量编排：固定计划、真实生命周期和失败项安全重试。
use crate::{
    batch_runner::{BatchExecutor, BatchRunner, WaitLimits},
    operation_gates::OperationPermit,
    workspace::WorkspaceEngine,
};
use essim_core::{
    application::{
        commands::batch_targets::{BatchPlan, BatchRequest, BatchScope},
        queries::{batch::BatchView, batch_plan::BatchPlanPreview},
    },
    domain::{
        aggregates::batch::{BatchOperation, ItemState, Outcome},
        error::Error,
        identity::EndpointId,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
/// 前端草稿是会话编辑状态，只用于显式跳过，不写入配置目录。
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftDisposition {
    /// 未保存配置涉及的端点（含引用未保存模型）。
    #[serde(default)]
    pub unsaved: Vec<EndpointId>,
    /// 主站待写草稿选择保留的端点；显式丢弃的项不在此列。
    #[serde(default)]
    pub retain_writes: Vec<EndpointId>,
}
/// 应用预览只返回迁移数量，不复制迁移后的全部点值。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetPreview {
    /// 端点稳定身份。
    pub endpoint: EndpointId,
    /// 当前运行状态；应用会停止此实例。
    pub state: String,
    /// 无法预览或草稿跳过原因。
    pub reason: Option<String>,
    /// 初始化、删除和倍率变化的点数摘要。
    pub changes: [usize; 3],
}
/// 单次固定计划和逐项预览。
#[derive(Serialize)]
pub struct LifecyclePreview {
    /// 固定目标摘要。
    pub plan: BatchPlanPreview,
    /// 当前状态与逐项跳过原因。
    pub items: Vec<TargetPreview>,
}
struct Prepared {
    created: Instant,
    plan: BatchPlan,
    _lease: crate::resources::Reservation,
    skipped: BTreeMap<EndpointId, String>,
}
struct LifecycleExecutor {
    engine: Arc<WorkspaceEngine>,
    skips: Arc<Mutex<BTreeMap<EndpointId, String>>>,
}
impl BatchExecutor for LifecycleExecutor {
    fn wait_limit(&self, plan: &BatchPlan, id: &EndpointId) -> Option<Duration> {
        plan.preview()
            .targets
            .iter()
            .find(|t| &t.endpoint == id)
            .map(|t| Duration::from_millis(t.wait_ms))
    }

    fn execute(&self, plan: &BatchPlan, id: &EndpointId, permit: &OperationPermit) -> Outcome {
        if let Some(reason) = self
            .skips
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(id)
            .cloned()
        {
            return Outcome::Skipped(reason);
        }
        match self.engine.run_batch_target(plan, id, permit) {
            Ok(Some(reason)) => Outcome::Skipped(reason),
            Ok(None) => Outcome::Succeeded,
            Err(e)
                if [
                    "BATCH_TARGET_CHANGED",
                    "BATCH_TARGET_DELETED",
                    "NOT_APPLIED",
                ]
                .contains(&e.code.as_str()) =>
            {
                Outcome::Skipped(e.message)
            }
            Err(e) if ["COMPLETION_LOST", "COMPLETION_UNCONFIRMED"].contains(&e.code.as_str()) => {
                Outcome::Unconfirmed(e.message)
            }
            Err(e) => Outcome::Failed(format!("{}: {}", e.code, e.message)),
        }
    }
}
/// 归属应用会话，关闭任务Tab不销毁执行器；仅保留一个待确认计划和当前任务。
pub struct BatchCoordinator {
    engine: Arc<WorkspaceEngine>,
    runner: BatchRunner,
    prepared: Mutex<Option<Prepared>>,
    skips: Arc<Mutex<BTreeMap<EndpointId, String>>>,
}
impl BatchCoordinator {
    /// 工程切换期间禁止新任务启动；未结束任务必须先结束或取消并等待。
    /// # Errors
    /// 批量任务仍活动、预览锁不可用或工程操作失败。
    pub fn change_project<T>(&self, change: impl FnOnce() -> Result<T, Error>) -> Result<T, Error> {
        let mut prepared = self
            .prepared
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "批量预览不可用"))?;
        if self.runner.query()?.is_some_and(|v| !v.finished) {
            return Err(Error::new("BATCH_BUSY", "请先结束批量任务再切换工程"));
        }
        let result = change()?;
        prepared.take();
        Ok(result)
    }

    /// 装配固定执行资源和工作区共同端点门。
    /// # Errors
    /// 固定线程创建失败或策略非法。
    pub fn new(engine: Arc<WorkspaceEngine>) -> Result<Self, Error> {
        let skips = Arc::new(Mutex::new(BTreeMap::new()));
        let runner = BatchRunner::with_operations(
            Arc::new(LifecycleExecutor {
                engine: engine.clone(),
                skips: skips.clone(),
            }),
            WaitLimits {
                apply: Duration::from_secs(10),
                start: Duration::from_secs(10),
                stop: Duration::from_secs(10),
            },
            engine.resources.policy().batch_targets,
            engine.resources.policy().preview_bytes as u64,
            engine.operation_gates(),
        )?;
        Ok(Self {
            engine,
            runner,
            prepared: Mutex::new(None),
            skips,
        })
    }
    /// 固定目标；不执行或保存，五分钟后须重新预览。
    /// # Errors
    /// 活动任务、范围非法、预览超限或内部状态不可用。
    pub fn preview(
        &self,
        request: BatchRequest,
        drafts: DraftDisposition,
    ) -> Result<LifecyclePreview, Error> {
        let mut prepared = self
            .prepared
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "预览不可用"))?;
        if self.runner.query()?.is_some_and(|v| !v.finished) {
            return Err(Error::new("BATCH_BUSY", "已有任务未结束"));
        }
        prepared.take();
        let (plan, lease) = {
            let w = self
                .engine
                .workspace
                .lock()
                .map_err(|_| Error::new("STATE_POISONED", "目录不可用"))?;
            let lease =
                self.engine
                    .resources
                    .reserve(essim_core::domain::resources::ResourceUsage {
                        memory_bytes: crate::resources::catalog_memory(w.catalog_ref())?,
                        ..Default::default()
                    })?;
            let plan = w.prepare_batch(
                request.id,
                request.operation,
                request.scope,
                request.concurrency,
            )?;
            (plan, lease)
        };
        let mut skipped = BTreeMap::new();
        let mut items = Vec::new();
        for target in &plan.preview().targets {
            let id = &target.endpoint;
            let mut reason = None;
            let mut changes = [0; 3];
            if plan.preview().operation == BatchOperation::Apply {
                if drafts.unsaved.contains(id) {
                    reason = Some("存在未保存配置草稿，已跳过".into());
                } else if drafts.retain_writes.contains(id) {
                    reason = Some("选择保留主站待写草稿，已跳过".into());
                } else {
                    match self.engine.preview_apply(id.clone(), false) {
                        Err(e) => reason = Some(format!("{}: {}", e.code, e.message)),
                        Ok(p) => {
                            for m in p.units.values() {
                                changes[0] += m.initialized.len();
                                changes[1] += m.removed.len();
                                changes[2] += m.scale_changed.len();
                            }
                        }
                    }
                }
            }
            if let Some(reason) = &reason {
                skipped.insert(id.clone(), reason.clone());
            }
            let state = self
                .engine
                .batch_target_state(id, target.role)
                .unwrap_or_else(|e| format!("状态查询失败: {}", e.message));
            items.push(TargetPreview {
                endpoint: id.clone(),
                state,
                reason,
                changes,
            });
        }
        let result = LifecyclePreview {
            plan: plan.preview().clone(),
            items,
        };
        if crate::resources::serialized_size(&result)? > 8 * 1024 * 1024 {
            return Err(Error::new("BATCH_MANIFEST_LIMIT", "预览过大"));
        }
        *prepared = Some(Prepared {
            created: Instant::now(),
            plan,
            _lease: lease,
            skipped,
        });
        Ok(result)
    }
    /// 消费已确认预览；只提交一次，重复提交拒绝。
    /// # Errors
    /// 预览过期、身份不匹配或任务仍活动。
    pub fn start(&self, id: &str) -> Result<(), Error> {
        let mut prepared = self
            .prepared
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "预览不可用"))?;
        let p = prepared
            .as_ref()
            .ok_or_else(|| Error::new("PREVIEW_EXPIRED", "请先预览"))?;
        if p.plan.preview().id != id || p.created.elapsed() > Duration::from_secs(300) {
            return Err(Error::new("PREVIEW_EXPIRED", "预览身份不匹配或过期"));
        }
        if self.runner.query()?.is_some_and(|v| !v.finished) {
            return Err(Error::new("BATCH_BUSY", "已有任务未结束"));
        }
        let p = prepared.take().unwrap();
        *self
            .skips
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "草稿处置不可用"))? = p.skipped;
        self.runner.submit_reserved(p.plan, p._lease)?;
        Ok(())
    }
    /// 只丢弃匹配的未提交预览，归还暂存；不会取消已开始任务。
    /// # Errors
    /// 预览锁不可用。
    pub fn discard_preview(&self, id: &str) -> Result<(), Error> {
        let mut p = self
            .prepared
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "预览不可用"))?;
        if p.as_ref().is_some_and(|p| p.plan.preview().id == id) {
            p.take();
        }
        Ok(())
    }
    /// 读取当前任务，重开页面可恢复同一会话进度。
    /// # Errors
    /// 执行器状态不可用。
    pub fn query(&self) -> Result<Option<BatchView>, Error> {
        self.runner.query()
    }
    /// 取消未开始项；不打断真实释放。
    /// # Errors
    /// 任务身份不匹配。
    pub fn cancel(&self, id: &str) -> Result<(), Error> {
        self.runner.cancel(id)
    }
    /// 仅对已结束任务的失败项生成新的固定预览；不自动发送。
    /// # Errors
    /// 前任务未确认、身份错误、没有失败项或目标已删除。
    pub fn retry(
        &self,
        previous: &str,
        id: String,
        drafts: DraftDisposition,
    ) -> Result<LifecyclePreview, Error> {
        let v = self
            .runner
            .query()?
            .ok_or_else(|| Error::new("BATCH_NOT_FOUND", "任务不存在"))?;
        if v.id != previous || !v.finished {
            return Err(Error::new("BATCH_BUSY", "前任务尚未确认结束或身份不匹配"));
        }
        let ids = v
            .items
            .iter()
            .filter(|i| i.state == ItemState::Failed)
            .map(|i| i.endpoint.clone())
            .collect();
        self.preview(
            BatchRequest {
                id,
                operation: v.operation,
                scope: BatchScope::Selected { ids },
                concurrency: 4,
            },
            drafts,
        )
    }
    /// 退出请求后等待固定执行线程完成；不伪称协议资源已释放。
    pub fn wait_idle(&self) {
        self.runner.wait_idle();
    }
    /// 会话退出停止领取后续项；调用方须随后等待执行器销毁再停协议所有者。
    /// # Errors
    /// 执行器状态不可用。
    pub fn request_shutdown(&self) -> Result<(), Error> {
        self.runner.request_shutdown()
    }
}
