//! 固定8线程的批量任务执行器；等待超时不代表后台动作结束。
use crate::operation_gates::{OperationGates, OperationPermit};
use essim_core::{
    application::{
        batch_service::BatchService,
        commands::{batch::BatchCommand, batch_targets::BatchPlan},
        events::batch::BatchFact,
        queries::batch::BatchView,
    },
    domain::{
        aggregates::batch::{BatchOperation, ItemState, Outcome},
        error::Error,
        identity::EndpointId,
    },
};
use std::{
    collections::BTreeMap,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
/// 各生命周期动作的观察等待阈值；实际动作的超时／完成由适配器判断。
#[derive(Clone, Copy)]
pub struct WaitLimits {
    /// 应用配置等待阈值。
    pub apply: Duration,
    /// 启动／连接等待阈值。
    pub start: Duration,
    /// 停止／释放等待阈值。
    pub stop: Duration,
}
impl WaitLimits {
    fn get(self, op: BatchOperation) -> Duration {
        match op {
            BatchOperation::Apply => self.apply,
            BatchOperation::Start => self.start,
            BatchOperation::Stop => self.stop,
        }
    }
}
/// 真实动作适配端口；暂未装配生产生命周期入口。
pub trait BatchExecutor: Send + Sync + 'static {
    /// 只从固定计划读取观察阈值，不能等待网络、目录锁或磁盘；缺省沿用执行器配置。
    fn wait_limit(&self, _plan: &BatchPlan, _endpoint: &EndpointId) -> Option<Duration> {
        None
    }

    /// 对固定目标执行一次操作，仅真实结束后返回结果。
    /// 实现必须在公共端点门内调用plan.revalidate，并等待后台真实完成。
    /// API等待超时而后台仍忙时不能返回Failed；应继续等待实际操作句柄。
    fn execute(&self, plan: &BatchPlan, endpoint: &EndpointId, permit: &OperationPermit)
        -> Outcome;
}
#[derive(Default)]
struct State {
    service: BatchService,
    plan: Option<Arc<BatchPlan>>,
    reservation: Option<crate::resources::Reservation>,
    active: BTreeMap<EndpointId, Instant>,
    stopping: bool,
    failure: Option<Error>,
}
struct Shared {
    state: Mutex<State>,
    wake: Condvar,
    executor: Arc<dyn BatchExecutor>,
    origin: Instant,
    waits: WaitLimits,
    operations: OperationGates,
}
impl Shared {
    fn now(&self) -> u64 {
        self.origin.elapsed().as_millis().min(u64::MAX as u128) as u64
    }
    fn expire(&self, state: &mut State) -> Result<(), Error> {
        let Some(view) = state.service.query(self.now()) else {
            return Ok(());
        };
        let default_limit = self.waits.get(view.operation);
        let expired = view
            .items
            .iter()
            .filter(|i| {
                i.state == ItemState::Running
                    && state.active.get(&i.endpoint).is_some_and(|t| {
                        t.elapsed()
                            >= state
                                .plan
                                .as_ref()
                                .and_then(|p| self.executor.wait_limit(p, &i.endpoint))
                                .unwrap_or(default_limit)
                    })
            })
            .map(|i| i.endpoint.clone())
            .collect::<Vec<_>>();
        for endpoint in expired {
            state.service.execute(
                BatchCommand::MarkUncertain {
                    id: view.id.clone(),
                    endpoint,
                    reason: "等待结果超时，实际操作未结束，继续占用执行槽".into(),
                },
                self.now(),
            )?;
        }
        Ok(())
    }
}
/// 会话级固定执行资源；离开页面不应销毁此对象。
pub struct BatchRunner {
    shared: Arc<Shared>,
    workers: Vec<JoinHandle<()>>,
    max_targets: usize,
    max_manifest_bytes: u64,
}
fn poisoned() -> Error {
    Error::new("STATE_POISONED", "批量执行状态不可用")
}
impl BatchRunner {
    /// 建立8个固定工作线程，不按任务数或超时增加线程。
    /// # Errors
    /// 等待阈值／清单限制非法，或工作线程创建失败；失败回收已创建线程。
    pub fn new(
        executor: Arc<dyn BatchExecutor>,
        waits: WaitLimits,
        max_targets: usize,
        max_manifest_bytes: u64,
    ) -> Result<Self, Error> {
        Self::with_operations(
            executor,
            waits,
            max_targets,
            max_manifest_bytes,
            OperationGates::default(),
        )
    }
    /// 与工作区共享同一公共端点门，手动入口和回调遵守同一占位。
    /// # Errors
    /// 参数非法或工作线程创建失败。
    pub fn with_operations(
        executor: Arc<dyn BatchExecutor>,
        waits: WaitLimits,
        max_targets: usize,
        max_manifest_bytes: u64,
        operations: OperationGates,
    ) -> Result<Self, Error> {
        if max_targets == 0
            || max_targets > 1024
            || max_manifest_bytes == 0
            || [waits.apply, waits.start, waits.stop].contains(&Duration::ZERO)
        {
            return Err(Error::new("INVALID_BATCH_LIMIT", "批量执行限制非法"));
        }
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            wake: Condvar::new(),
            executor,
            origin: Instant::now(),
            waits,
            operations,
        });
        let mut workers = Vec::new();
        for index in 0..8 {
            let state = shared.clone();
            match thread::Builder::new()
                .name(format!("essim-batch-{index}"))
                .spawn(move || worker(state))
            {
                Ok(handle) => workers.push(handle),
                Err(error) => {
                    if let Ok(mut s) = shared.state.lock() {
                        s.stopping = true;
                    }
                    shared.wake.notify_all();
                    for worker in workers {
                        let _ = worker.join();
                    }
                    return Err(Error::new("THREAD_CREATE_FAILED", error.to_string()));
                }
            }
        }
        Ok(Self {
            shared,
            workers,
            max_targets,
            max_manifest_bytes,
        })
    }
    /// 接受一个固定清单；已占用任务包含尚未结束的未确认项。
    /// # Errors
    /// 超出清单数量／字节限制、活动任务未结束或内部状态不可用。
    pub fn submit(&self, plan: BatchPlan) -> Result<(), Error> {
        self.submit_inner(plan, None)
    }
    /// 随任务保留模型比较暂存额度，只有真实完成所有项才归还。
    /// # Errors
    /// 活动任务、清单超限或状态不可用。
    pub fn submit_reserved(
        &self,
        plan: BatchPlan,
        reservation: crate::resources::Reservation,
    ) -> Result<(), Error> {
        self.submit_inner(plan, Some(reservation))
    }
    fn submit_inner(
        &self,
        plan: BatchPlan,
        reservation: Option<crate::resources::Reservation>,
    ) -> Result<(), Error> {
        if plan.preview().targets.len() > self.max_targets
            || crate::resources::serialized_size(plan.preview())? > self.max_manifest_bytes
        {
            return Err(Error::new("BATCH_MANIFEST_LIMIT", "批量清单数量或字节超限"));
        }
        let mut state = self.shared.state.lock().map_err(|_| poisoned())?;
        if let Some(error) = &state.failure {
            return Err(error.clone());
        }
        if state.stopping {
            return Err(Error::new("BATCH_STOPPING", "批量执行器正在退出"));
        }
        state
            .service
            .execute(plan.start_command(), self.shared.now())?;
        if let Some(charge) = &reservation {
            charge.commit();
        }
        state.reservation = reservation;
        state.plan = Some(Arc::new(plan));
        drop(state);
        self.shared.wake.notify_all();
        Ok(())
    }
    /// 取消尚未领取项；不打断已经开始的动作或释放过程。
    /// # Errors
    /// 任务身份不匹配或内部状态不可用。
    pub fn cancel(&self, id: &str) -> Result<(), Error> {
        let mut state = self.shared.state.lock().map_err(|_| poisoned())?;
        state
            .service
            .execute(BatchCommand::Cancel { id: id.into() }, self.shared.now())?;
        let retired = retire_finished(&mut state, self.shared.now());
        drop(state);
        drop(retired);
        self.shared.wake.notify_all();
        Ok(())
    }
    /// 只查有界任务摘要，不等待执行器／网络／磁盘；查询时推进超时观察状态。
    /// # Errors
    /// 内部状态不可用或事实投影失败。
    pub fn query(&self) -> Result<Option<BatchView>, Error> {
        let mut state = self.shared.state.lock().map_err(|_| poisoned())?;
        if let Some(error) = &state.failure {
            return Err(error.clone());
        }
        self.shared.expire(&mut state)?;
        Ok(state.service.query(self.shared.now()))
    }
    /// 请求应用退出时停止领取新项；已开始项不被中断，销毁时等待真实结束。
    /// 此方法返回不代表已释放所有资源，仍须查询任务并等待所有者回收。
    /// # Errors
    /// 内部状态不可用或任务取消事实失败。
    pub fn request_shutdown(&self) -> Result<(), Error> {
        let mut state = self.shared.state.lock().map_err(|_| poisoned())?;
        if state.stopping {
            return Ok(());
        }
        state.stopping = true;
        let result = if let Some(view) = state.service.query(self.shared.now()) {
            state
                .service
                .execute(BatchCommand::Cancel { id: view.id }, self.shared.now())
                .map(|_| ())
        } else {
            Ok(())
        };
        let retired = retire_finished(&mut state, self.shared.now());
        drop(state);
        drop(retired);
        self.shared.wake.notify_all();
        result
    }
    /// 等待已开始执行的固定线程离开；退出请求先停止领取。
    /// 无法确认的适配器故障在退出时由协议所有者统一回收。
    pub fn wait_idle(&self) {
        for worker in &self.workers {
            while !worker.is_finished() {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
    /// 已创建的固定工作线程数，不随目标数量增长。
    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }
}
fn retire_finished(
    state: &mut State,
    now: u64,
) -> (
    Option<Arc<BatchPlan>>,
    Option<crate::resources::Reservation>,
) {
    if state.service.query(now).is_some_and(|v| v.finished) {
        (state.plan.take(), state.reservation.take())
    } else {
        (None, None)
    }
}
fn claim(shared: &Shared) -> Option<(Arc<BatchPlan>, EndpointId)> {
    let mut state = shared.state.lock().ok()?;
    loop {
        if state.stopping || state.failure.is_some() {
            return None;
        }
        if let Err(error) = shared.expire(&mut state) {
            state.failure = Some(error);
            return None;
        }
        if let Some(plan) = state.plan.clone() {
            match state.service.execute(
                BatchCommand::Claim {
                    id: plan.preview().id.clone(),
                },
                shared.now(),
            ) {
                Ok(Some(event)) => {
                    if let BatchFact::ItemStarted { endpoint } = event.fact {
                        state.active.insert(endpoint.clone(), Instant::now());
                        return Some((plan, endpoint));
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    state.failure = Some(error);
                    return None;
                }
            }
        }
        state = shared.wake.wait(state).ok()?;
    }
}
fn worker(shared: Arc<Shared>) {
    while let Some((plan, endpoint)) = claim(&shared) {
        let (mut permit, refused) = match shared.operations.acquire(&endpoint) {
            Ok(p) => (Some(p), None),
            Err(e) => (None, Some(e)),
        };
        let result = if let Some(error) = refused {
            Ok(Ok(Outcome::Skipped(error.message)))
        } else {
            catch_unwind(AssertUnwindSafe(|| {
                let Some(permit) = permit.as_ref() else {
                    return Err(Error::new("OPERATION_PERMIT_MISSING", "操作凭证缺失"));
                };
                let outcome = shared.executor.execute(&plan, &endpoint, permit);
                if let Outcome::Unconfirmed(reason) = outcome {
                    return Err(Error::new("COMPLETION_UNCONFIRMED", reason));
                }
                permit.wait_for_callbacks()?;
                Ok(outcome)
            }))
        };
        let unknown_reason = match &result {
            Ok(Err(error)) => format!("回调完成未确认：{}；保留槽位，禁止重复执行", error.message),
            _ => "执行适配器panic，无法确认动作结束；保留槽位，禁止重复执行".into(),
        };
        let Ok(mut state) = shared.state.lock() else {
            return;
        };
        match result {
            Ok(Ok(outcome)) => {
                drop(permit.take());
                match state.service.execute(
                    BatchCommand::Complete {
                        id: plan.preview().id.clone(),
                        endpoint: endpoint.clone(),
                        outcome,
                    },
                    shared.now(),
                ) {
                    Ok(_) => {
                        state.active.remove(&endpoint);
                    }
                    Err(error) => {
                        state.failure = Some(error);
                    }
                }
            }
            Err(_) | Ok(Err(_)) => {
                let result = state.service.execute(
                    BatchCommand::MarkUncertain {
                        id: plan.preview().id.clone(),
                        endpoint,
                        reason: unknown_reason,
                    },
                    shared.now(),
                );
                if let Err(error) = result {
                    state.failure = Some(error);
                }
                // 不把panic当作动作已结束，也不通过替换线程增加执行能力。
                while !state.stopping {
                    let Ok(next) = shared.wake.wait(state) else {
                        return;
                    };
                    state = next;
                }
                return;
            }
        }
        let retired = retire_finished(&mut state, shared.now());
        drop(state);
        drop(retired);
        shared.wake.notify_all();
    }
}
impl Drop for BatchRunner {
    fn drop(&mut self) {
        let _ = self.request_shutdown();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod reservation_tests {
    use super::*;
    #[test]
    fn cancellation_before_first_claim_retires_task_memory_without_starting_an_adapter() {
        let manager = crate::resources::ResourceManager::default();
        let charge = manager
            .reserve(essim_core::domain::resources::ResourceUsage {
                memory_bytes: 64,
                ..Default::default()
            })
            .unwrap();
        charge.commit();
        let mut state = State::default();
        state.reservation = Some(charge);
        state
            .service
            .execute(
                BatchCommand::Start {
                    id: "pending".into(),
                    operation: BatchOperation::Stop,
                    targets: vec![EndpointId("a".into())],
                    concurrency: 1,
                },
                0,
            )
            .unwrap();
        assert!(retire_finished(&mut state, 0).1.is_none());
        assert_eq!(manager.query().unwrap().current.memory_bytes, 64);
        state
            .service
            .execute(
                BatchCommand::Cancel {
                    id: "pending".into(),
                },
                1,
            )
            .unwrap();
        let retired = retire_finished(&mut state, 1);
        assert_eq!(state.service.query(1).unwrap().counts.cancelled, 1);
        drop(retired);
        assert_eq!(manager.query().unwrap().current.memory_bytes, 0);
    }
}
