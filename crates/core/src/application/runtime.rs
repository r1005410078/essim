//! 运行、可靠录制和隔离回放契约。生产协议及持久化适配器由后续阶段提供。
use crate::domain::{
    error::Error,
    identity::{EndpointId, ReplayId, RunId, UnitId},
    model::CollectionModel,
    values::UnitValues,
};
/// 运行事实与历史事件的兼容导出。
pub use super::events::history::{Fact, HistoryEvent};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 固定一次运行的配置、模型快照和初始状态。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSnapshot {
    /// 每次显式启动使用新身份。
    pub run: RunId,
    /// 运行资源边界。
    pub endpoint: EndpointId,
    /// 该运行的全部单元，故障停止不只停止当前选中单元。
    pub units: Vec<UnitId>,
    /// 固定不可变模型快照，不跟随模型后续保存。
    pub models: Vec<CollectionModel>,
    /// 起始值检查点。
    pub initial_values: BTreeMap<UnitId, UnitValues>,
    /// 规范化的物理串口身份；执行器仍须处理别名及系统独占。
    pub serial_path: Option<String>,
}
/// 实际外部操作的技术结果，不等于业务动作完成。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// 协议层是否确认。
    pub confirmed: bool,
    /// 原因或协议结果。
    pub detail: String,
}
/// 执行器独占连接及运行值；停止请求与资源释放确认分开。
pub trait ProtocolExecutor {
    /// 打开通道，成功表示执行器已完成打开。
    /// # Errors
    /// 连接或资源获取失败。
    fn open(&mut self, snapshot: &RunSnapshot) -> Result<(), Error>;
    /// 执行一次带操作身份的外部操作，不允许内部自动重发控制。
    /// # Errors
    /// 通信失败或结果未知。
    fn execute(&mut self, run: &RunId, operation: &str) -> Result<ExecutionResult, Error>;
    /// 请求停止全部下属单元并释放通道；返回不代表释放已确认。
    /// # Errors
    /// 无法发出停止请求。
    fn request_stop(&mut self, run: &RunId) -> Result<(), Error>;
}
/// 可靠录制端口；成功必须代表可靠确认，不能只代表进入内存队列。
pub trait RecorderPort {
    /// 可靠建立含配置快照及初始值的运行记录。
    /// # Errors
    /// 创建或可靠写入失败。
    fn create(&mut self, snapshot: &RunSnapshot) -> Result<(), Error>;
    /// 可靠追加有序事实。
    /// # Errors
    /// 写入失败；调用方必须停止受影响运行。
    fn append(&mut self, event: &HistoryEvent) -> Result<(), Error>;
}
/// 时间端口，区分单调相对时间与展示用墙钟。
pub trait Clock {
    /// 单调时钟，毫秒。
    fn monotonic_ms(&self) -> u64;
    /// Unix墙钟，毫秒；不能用作事件排序。
    fn unix_ms(&self) -> u64;
}
/// 历史只读端口，不持有协议执行器。
pub trait HistoryReader {
    /// 读取固定运行及有序记录。
    /// # Errors
    /// 历史不可用、损坏或版本不支持。
    fn read(&self, run: &RunId) -> Result<(RunSnapshot, Vec<HistoryEvent>), Error>;
}
/// 资源生命周期，停止确认前不能呈现Stopped。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunLifecycle {
    /// 记录建立或资源打开中。
    Starting,
    /// 已运行。
    Running,
    /// 资源释放尚待确认。
    Stopping,
    /// 已确认停止。
    Stopped,
    /// 打开前失败或释放超时。
    Failed,
}
/// 录制生命周期与连接状态分离。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingState {
    /// 准备记录。
    Preparing,
    /// 可靠记录中。
    Recording,
    /// 已可靠写入结束标记。
    Finalized,
    /// 记录失败或未完整结束。
    Failed,
}
/// 一次运行的控制状态；实际值由执行器维护。
#[derive(Debug, Clone)]
pub struct RunSession {
    /// 固定运行身份和配置快照。
    pub snapshot: RunSnapshot,
    /// 资源状态。
    pub lifecycle: RunLifecycle,
    /// 录制状态。
    pub recording: RecordingState,
    /// 最后可靠记录位置。
    pub last_reliable: u64,
    /// 独立于失败记录存储的可展示错误。
    pub fault: Option<Error>,
    /// 已执行但结果未可靠确认，不能重发。
    pub uncertain: bool,
    /// 连接变化不改变运行身份。
    pub connected: bool,
    /// 暂停单元集合。
    pub paused: BTreeSet<UnitId>,
    started_ms: u64,
    stopping_ms: Option<u64>,
    attempted: BTreeSet<String>,
}
/// 单进程运行协调器；M01生产装配不注入测试替身。
pub struct RunCoordinator<E, R, C> {
    executor: E,
    recorder: R,
    clock: C,
    sessions: BTreeMap<RunId, RunSession>,
}
impl<E: ProtocolExecutor, R: RecorderPort, C: Clock> RunCoordinator<E, R, C> {
    /// 注入窄端口，不决定协议线程调度方式。
    pub fn new(executor: E, recorder: R, clock: C) -> Self {
        Self {
            executor,
            recorder,
            clock,
            sessions: BTreeMap::new(),
        }
    }
    /// 查询单次运行控制状态，不执行通信。
    pub fn get(&self, run: &RunId) -> Option<&RunSession> {
        self.sessions.get(run)
    }
    /// 先建立可靠记录，再打开执行器。
    /// # Errors
    /// 重用RunId、端点／串口被占用、录制创建或打开失败时返回错误。
    pub fn start(&mut self, snapshot: RunSnapshot) -> Result<(), Error> {
        if snapshot.run.0.is_empty() || self.sessions.contains_key(&snapshot.run) {
            return Err(Error::new("RUN_ID_REUSED", "每次显式启动需要新的RunId"));
        }
        if self.sessions.values().any(|session| {
            session.lifecycle != RunLifecycle::Stopped
                // 创建记录失败时尚未触碰执行器，不应永久占用端点或串口。
                && !(session.lifecycle == RunLifecycle::Failed
                    && session.stopping_ms.is_none())
                && (session.snapshot.endpoint == snapshot.endpoint
                    || snapshot
                        .serial_path
                        .as_ref()
                        .is_some_and(|path| session.snapshot.serial_path.as_ref() == Some(path)))
        }) {
            return Err(Error::new("RESOURCE_BUSY", "端点或串口仍被运行占用"));
        }
        let id = snapshot.run.clone();
        self.sessions.insert(
            id.clone(),
            RunSession {
                snapshot: snapshot.clone(),
                lifecycle: RunLifecycle::Starting,
                recording: RecordingState::Preparing,
                last_reliable: 0,
                fault: None,
                uncertain: false,
                connected: false,
                paused: BTreeSet::new(),
                started_ms: self.clock.monotonic_ms(),
                stopping_ms: None,
                attempted: BTreeSet::new(),
            },
        );
        if let Err(error) = self.recorder.create(&snapshot) {
            let session = self.sessions.get_mut(&id).expect("刚插入的运行存在");
            session.lifecycle = RunLifecycle::Failed;
            session.recording = RecordingState::Failed;
            session.fault = Some(error.clone());
            return Err(error);
        }
        self.sessions.get_mut(&id).expect("运行存在").recording = RecordingState::Recording;
        if let Err(error) = self.executor.open(&snapshot) {
            self.fail(&id, error.clone());
            return Err(error);
        }
        self.append(&id, Fact::Opened, None)?;
        let session = self.sessions.get_mut(&id).expect("运行存在");
        session.lifecycle = RunLifecycle::Running;
        session.connected = true;
        Ok(())
    }
    /// 外部效果门禁：意图可靠 → 执行一次 → 结果可靠；不自动重试。
    /// # Errors
    /// 非运行中、重复操作、记录或协议失败时拒绝；结果写入失败标记未确认。
    pub fn perform(&mut self, run: &RunId, operation: &str) -> Result<ExecutionResult, Error> {
        let session = self
            .sessions
            .get_mut(run)
            .ok_or_else(|| Error::new("RUN_NOT_FOUND", "运行不存在"))?;
        if session.lifecycle != RunLifecycle::Running
            || session.recording != RecordingState::Recording
        {
            return Err(Error::new("RUN_UNAVAILABLE", "运行当前不能接受新操作"));
        }
        if operation.trim().is_empty() || !session.attempted.insert(operation.into()) {
            return Err(Error::new(
                "DUPLICATE_OPERATION",
                "操作身份为空或已执行；禁止自动重发",
            ));
        }
        self.append(
            run,
            Fact::Intent {
                operation: operation.into(),
            },
            Some(operation.into()),
        )?;
        let outcome = self.executor.execute(run, operation);
        let result = outcome.clone().unwrap_or_else(|error| ExecutionResult {
            confirmed: false,
            detail: error.to_string(),
        });
        // 执行后结果未落盘时，无论底层是否收到响应都不能承诺可靠成功。
        self.sessions.get_mut(run).expect("运行存在").uncertain = true;
        self.append(
            run,
            Fact::Result {
                operation: operation.into(),
                result: result.clone(),
            },
            Some(operation.into()),
        )?;
        self.sessions.get_mut(run).expect("运行存在").uncertain = !result.confirmed;
        outcome
    }
    /// 请求停止，幂等且不提前确认释放。
    /// # Errors
    /// 运行不存在或无法发出停止请求。
    pub fn stop(&mut self, run: &RunId) -> Result<(), Error> {
        let session = self
            .sessions
            .get_mut(run)
            .ok_or_else(|| Error::new("RUN_NOT_FOUND", "运行不存在"))?;
        if session.stopping_ms.is_some() || session.lifecycle == RunLifecycle::Stopped {
            return Ok(());
        }
        if session.lifecycle == RunLifecycle::Failed && session.last_reliable == 0 {
            return Ok(());
        }
        session.lifecycle = RunLifecycle::Stopping;
        session.stopping_ms = Some(self.clock.monotonic_ms());
        if let Err(error) = self.executor.request_stop(run) {
            session.fault = Some(error.clone());
            return Err(error);
        }
        Ok(())
    }
    /// 接受执行器资源释放确认，之后才显示停止。
    /// # Errors
    /// 状态非法或可靠结束标记写入失败；后者仍确认资源已释放但保留记录失败。
    pub fn confirm_stopped(&mut self, run: &RunId) -> Result<(), Error> {
        let session = self
            .sessions
            .get(run)
            .ok_or_else(|| Error::new("RUN_NOT_FOUND", "运行不存在"))?;
        if session.lifecycle == RunLifecycle::Stopped {
            return Ok(());
        }
        if session.stopping_ms.is_none() {
            return Err(Error::new("INVALID_STATE", "尚未请求停止"));
        }
        let result = if session.recording == RecordingState::Recording {
            self.append(run, Fact::Finalized, None)
        } else {
            Ok(())
        };
        let session = self.sessions.get_mut(run).expect("运行存在");
        session.lifecycle = RunLifecycle::Stopped;
        session.connected = false;
        if result.is_ok() && session.recording == RecordingState::Recording {
            session.recording = RecordingState::Finalized;
        }
        result
    }
    /// 临时重连只记录连接事实，保持原RunId。
    /// # Errors
    /// 状态无效或录制失败。
    pub fn connection(&mut self, run: &RunId, connected: bool) -> Result<(), Error> {
        self.require_running(run)?;
        self.append(run, Fact::Connection { connected }, None)?;
        self.sessions.get_mut(run).expect("运行存在").connected = connected;
        Ok(())
    }
    /// 暂停单元采集的状态事实，不创建新运行。
    /// # Errors
    /// 单元不属于运行、状态无效或录制失败。
    pub fn pause_unit(&mut self, run: &RunId, unit: UnitId, paused: bool) -> Result<(), Error> {
        self.require_running(run)?;
        if !self.sessions[run].snapshot.units.contains(&unit) {
            return Err(Error::new("UNIT_NOT_FOUND", "单元不属于该运行"));
        }
        self.append(
            run,
            Fact::UnitPaused {
                unit: unit.clone(),
                paused,
            },
            None,
        )?;
        let session = self.sessions.get_mut(run).expect("运行存在");
        if paused {
            session.paused.insert(unit);
        } else {
            session.paused.remove(&unit);
        }
        Ok(())
    }
    /// 共享记录存储故障逐一停止所有受影响运行，不因一个停止失败中断其他项。
    pub fn storage_failed(&mut self, runs: &[RunId], error: Error) {
        for run in runs {
            self.fail(run, error.clone());
        }
    }
    /// 停机确认超时后呈现失败，继续保留资源占用直到确认，不伪造已停止。
    pub fn check_stop_deadlines(&mut self, timeout_ms: u64) {
        for session in self.sessions.values_mut() {
            if session.lifecycle == RunLifecycle::Stopping
                && session.stopping_ms.is_some_and(|start| {
                    self.clock.monotonic_ms().saturating_sub(start) >= timeout_ms
                })
            {
                session.lifecycle = RunLifecycle::Failed;
                session.fault = Some(Error::new("STOP_TIMEOUT", "等待资源释放确认超时"));
            }
        }
    }
    fn require_running(&self, run: &RunId) -> Result<(), Error> {
        if self.sessions.get(run).is_some_and(|session| {
            session.lifecycle == RunLifecycle::Running
                && session.recording == RecordingState::Recording
        }) {
            Ok(())
        } else {
            Err(Error::new("RUN_UNAVAILABLE", "运行当前不可操作"))
        }
    }
    fn append(&mut self, run: &RunId, fact: Fact, command_id: Option<String>) -> Result<(), Error> {
        let session = self
            .sessions
            .get(run)
            .ok_or_else(|| Error::new("RUN_NOT_FOUND", "运行不存在"))?;
        let event = HistoryEvent {
            schema: 1,
            run: run.clone(),
            sequence: session.last_reliable + 1,
            relative_ms: self.clock.monotonic_ms().saturating_sub(session.started_ms),
            unix_ms: self.clock.unix_ms(),
            command_id,
            fact,
        };
        if let Err(error) = self.recorder.append(&event) {
            self.fail(run, error.clone());
            return Err(error);
        }
        self.sessions.get_mut(run).expect("运行存在").last_reliable = event.sequence;
        Ok(())
    }
    fn fail(&mut self, run: &RunId, error: Error) {
        if let Some(session) = self.sessions.get_mut(run) {
            session.recording = RecordingState::Failed;
            session.fault = Some(error);
        }
        // 不再次向故障存储写停止错误，避免递归故障停机。
        let _ = self.stop(run);
    }
}
/// 独立回放状态；类型中没有ProtocolExecutor，不能重发命令。
#[derive(Debug, Clone)]
pub struct ReplaySession {
    /// 回放身份。
    pub id: ReplayId,
    /// 原运行的配置快照。
    pub snapshot: RunSnapshot,
    /// 最后验证通过的位置。
    pub position: u64,
    /// 隔离点值副本。
    pub values: BTreeMap<UnitId, UnitValues>,
    /// 没有可靠结果的操作身份。
    pub unconfirmed: BTreeSet<String>,
    /// 是否有可靠结束标记。
    pub finalized: bool,
    /// 损坏或不兼容时保留原因并停止重建。
    pub fault: Option<Error>,
}
impl ReplaySession {
    /// 从固定起始检查点开始，不引用实时实例可变状态。
    pub fn new(id: ReplayId, snapshot: RunSnapshot) -> Self {
        Self {
            id,
            values: snapshot.initial_values.clone(),
            snapshot,
            position: 0,
            unconfirmed: BTreeSet::new(),
            finalized: false,
            fault: None,
        }
    }
    /// 应用历史事实；不会执行Intent中的外部效果。
    /// # Errors
    /// 未知schema、错误RunId、序号缺口、未知点或结束后的事件时停止。
    pub fn accept(&mut self, event: &HistoryEvent) -> Result<bool, Error> {
        if let Some(error) = &self.fault {
            return Err(error.clone());
        }
        let result = self.apply_event(event);
        if let Err(error) = &result {
            self.fault = Some(error.clone());
        }
        result
    }
    fn apply_event(&mut self, event: &HistoryEvent) -> Result<bool, Error> {
        if event.schema != 1 || event.run != self.snapshot.run {
            return Err(Error::new("UNKNOWN_HISTORY", "历史版本或运行身份不符"));
        }
        if event.sequence <= self.position {
            return Ok(false);
        }
        if event.sequence != self.position + 1 || self.finalized {
            return Err(Error::new("HISTORY_GAP", "历史缺口或结束后仍有记录"));
        }
        match &event.fact {
            Fact::Intent { operation } => {
                self.unconfirmed.insert(operation.clone());
            }
            Fact::Result { operation, .. } => {
                if !self.unconfirmed.remove(operation) {
                    return Err(Error::new("MISSING_INTENT", "结果缺少对应意图"));
                }
            }
            Fact::Value { unit, point, raw } => {
                let values = self
                    .values
                    .get_mut(unit)
                    .ok_or_else(|| Error::new("UNIT_NOT_FOUND", "历史单元不存在"))?;
                let value = values
                    .raw
                    .get_mut(point)
                    .ok_or_else(|| Error::new("POINT_NOT_FOUND", "固定模型快照中不存在此点"))?;
                *value = raw.clone();
            }
            Fact::Finalized => self.finalized = true,
            _ => {}
        }
        self.position = event.sequence;
        Ok(true)
    }
}
