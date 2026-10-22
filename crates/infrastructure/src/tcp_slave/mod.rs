//! TCP从站适配器：固定工作线程、线程内C句柄、独立运行查询与可靠文件记录。
mod journal;
mod mapping;
mod worker;
use crate::operation_gates::{OperationGates, OperationPermit};
use essim_core::{
    application::{
        commands::slave::{SlaveAction, SlaveCommand},
        queries::slave::{SlaveQuery, SlaveView},
    },
    domain::{
        endpoint::EndpointConfig,
        error::Error,
        identity::{EndpointId, UnitId},
        values::UnitValues,
    },
};
use mio::{Poll, Token, Waker};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        mpsc::{self, SyncSender},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};
/// 一次启动固定的完整已应用配置与各Unit值。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// 已应用端点，不读取编辑草稿。
    pub endpoint: EndpointConfig,
    /// 已应用模型与独立初始运行值。
    pub values: BTreeMap<UnitId, UnitValues>,
}
fn timeout_error() -> Error {
    Error::new(
        "RUNTIME_TIMEOUT",
        "运行操作未在时限内确认；完成句柄可继续等待，不能推断失败或重新发送",
    )
}
/// 所有者真实回复的一次性接收端；超时后可继续等待，同一命令只入队一次。
pub struct SlaveCompletion {
    receiver: std::sync::mpsc::Receiver<Result<Reply, Error>>,
    finished: bool,
}
impl SlaveCompletion {
    /// 有限等待；None表示仍待确认，Some内才是实际执行结果。
    /// # Errors
    /// 句柄已消费或回复通道丢失；通道丢失不能当作动作未发生。
    pub fn wait_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<Result<SlaveView, Error>>, Error> {
        if self.finished {
            return Err(Error::new("COMPLETION_CONSUMED", "完成结果已消费"));
        }
        match self.receiver.recv_timeout(timeout) {
            Ok(result) => {
                self.finished = true;
                Ok(Some(result.and_then(|reply| match reply {
                    Reply::View(view) => Ok(view),
                    _ => Err(Error::new("UNEXPECTED_REPLY", "操作回复类型不匹配")),
                })))
            }
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(Error::new(
                "COMPLETION_LOST",
                "操作回复通道丢失，结果未确认，不能直接重复执行",
            )),
        }
    }
}
pub(super) fn failure(error: impl std::fmt::Display) -> Error {
    Error::new("TCP_RUNTIME_FAILED", error.to_string())
}
pub(super) enum Request {
    Start(Snapshot, usize, String),
    Stop(EndpointId, String),
    Values(SlaveCommand),
    Snapshot(EndpointId),
    Forget(EndpointId),
}
pub(super) enum Reply {
    View(SlaveView),
    Snapshot(Option<BTreeMap<UnitId, UnitValues>>),
}
pub(super) struct Message {
    endpoint: EndpointId,
    request: Request,
    reply: SyncSender<Result<Reply, Error>>,
    permit: Option<OperationPermit>,
    _queue: Option<crate::resources::Reservation>,
}
struct Worker {
    tx: SyncSender<Message>,
    wake: Arc<Waker>,
    join: Mutex<Option<JoinHandle<()>>>,
}
/// 固定工作线程的运行服务；网络及C资源均在所属工作线程内访问。
pub struct TcpSlaveRuntime {
    workers: Vec<Worker>,
    cache: worker::Cache,
    recording: crate::recording::RecordingPool,
    operations: OperationGates,
    resources: crate::resources::ResourceManager,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
    gates: Mutex<BTreeMap<EndpointId, Arc<Mutex<()>>>>,
}
impl TcpSlaveRuntime {
    /// 创建最多4个工作线程，实际端点启动前不占用外部监听端口。
    /// # Errors
    /// poll、唤醒器或线程创建失败。
    pub fn new(root: PathBuf) -> Result<Self, Error> {
        Self::with_operations(root, OperationGates::default())
    }
    /// 使用与工作区和批量调度相同的端点操作门。
    /// # Errors
    /// poll、唤醒器或工作线程创建失败。
    pub fn with_operations(root: PathBuf, operations: OperationGates) -> Result<Self, Error> {
        Self::with_resources(
            root,
            operations,
            crate::resources::ResourceManager::default(),
        )
    }
    /// 共享会话资源准入，手动与批量启动使用相同预算。
    /// # Errors
    /// 工作线程或系统轮询器创建失败。
    pub fn with_resources(
        root: PathBuf,
        operations: OperationGates,
        resources: crate::resources::ResourceManager,
    ) -> Result<Self, Error> {
        let recording = crate::recording::RecordingPool::new(resources.clone())?;
        Self::with_recording(root, operations, resources, recording)
    }
    /// 装配共享固定录制池，协议所有者与磁盘执行资源分别有界。
    /// # Errors
    /// 固定协议线程创建失败。
    pub fn with_recording(
        root: PathBuf,
        operations: OperationGates,
        resources: crate::resources::ResourceManager,
        recording: crate::recording::RecordingPool,
    ) -> Result<Self, Error> {
        let count = std::thread::available_parallelism()
            .map_or(1, |v| v.get())
            .min(4);
        let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut runtime = Self {
            workers: Vec::new(),
            cache: Default::default(),
            recording: recording.clone(),
            operations,
            resources: resources.clone(),
            shutdown: shutdown.clone(),
            gates: Mutex::new(BTreeMap::new()),
        };
        let global_clients = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        for index in 0..count {
            let poll = Poll::new().map_err(failure)?;
            let wake = Arc::new(Waker::new(poll.registry(), Token(0)).map_err(failure)?);
            let (tx, rx) = mpsc::sync_channel(256);
            let (ready, initialized) = mpsc::sync_channel(1);
            let root = root.clone();
            let done = shutdown.clone();
            let clients = global_clients.clone();
            let resources = resources.clone();
            let recording = recording.clone();
            let cache = runtime.cache.clone();
            let join = std::thread::Builder::new()
                .name(format!("modbus-tcp-{index}"))
                .spawn(move || {
                    worker::run(
                        poll, rx, root, done, clients, ready, resources, recording, cache,
                    )
                })
                .map_err(failure)?;
            runtime.workers.push(Worker {
                tx,
                wake,
                join: Mutex::new(Some(join)),
            });
            initialized
                .recv_timeout(Duration::from_secs(5))
                .map_err(failure)??;
        }
        Ok(runtime)
    }
    /// 停止所有工作线程并等待真实资源回收；可重复调用。
    pub fn shutdown(&self) {
        self.shutdown
            .store(true, std::sync::atomic::Ordering::Release);
        for worker in &self.workers {
            let _ = worker.wake.wake();
        }
        for worker in &self.workers {
            if let Ok(mut join) = worker.join.lock() {
                if let Some(join) = join.take() {
                    let _ = join.join();
                }
            }
        }
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }
    /// 固定录制资源的只读积压查询。
    /// # Errors
    /// 录制状态不可用。
    pub fn recording_view(&self) -> Result<crate::recording::RecordingView, Error> {
        self.recording.query()
    }
    /// 获取单端点操作门，配置应用与启动共享，等待不阻塞其他端点。
    /// # Errors
    /// 状态锁中毒。
    pub fn gate(&self, id: &EndpointId) -> Result<Arc<Mutex<()>>, Error> {
        Ok(self
            .gates
            .lock()
            .map_err(failure)?
            .entry(id.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone())
    }
    fn submit(
        &self,
        id: &EndpointId,
        request: Request,
        parent: Option<&OperationPermit>,
    ) -> Result<std::sync::mpsc::Receiver<Result<Reply, Error>>, Error> {
        let permit = if matches!(&request, Request::Snapshot(_)) {
            None
        } else {
            Some(self.operations.enter(id, parent)?)
        };
        let queue_bytes = match &request {
            Request::Start(snapshot, _, command) => {
                crate::resources::serialized_size(snapshot)?.saturating_add(command.len() as u64)
            }
            Request::Values(command) => crate::resources::serialized_size(command)?,
            // 查询和释放保留入口，不依赖可耗尽的数据队列字节额度。
            _ => 0,
        };
        let queue = if queue_bytes > 0 {
            Some(
                self.resources
                    .reserve(essim_core::domain::resources::ResourceUsage {
                        queue_bytes,
                        memory_bytes: queue_bytes,
                        ..Default::default()
                    })?,
            )
        } else {
            None
        };
        let index = id.0.bytes().fold(0usize, |v, b| {
            v.wrapping_mul(31).wrapping_add(usize::from(b))
        }) % self.workers.len();
        let worker = &self.workers[index];
        let (tx, rx) = mpsc::sync_channel(1);
        worker
            .tx
            .try_send(Message {
                endpoint: id.clone(),
                request,
                reply: tx,
                permit,
                _queue: queue,
            })
            .map_err(|_| Error::new("RUNTIME_BUSY", "运行命令队列已满或执行器不可用"))?;
        // 入队后不能将唤醒失败当作未提交；所有者轮询仍可消费，结果以完成句柄为准。
        let _ = worker.wake.wake();
        Ok(rx)
    }
    fn send(&self, id: &EndpointId, request: Request) -> Result<Reply, Error> {
        self.submit(id, request, None)?
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| timeout_error())?
    }
    /// 在所有者线程启动快照或处理其他运行操作。
    /// # Errors
    /// 参数非法、记录／监听失败、队列过载或操作未确认。
    pub fn execute(
        &self,
        command: SlaveCommand,
        snapshot: Option<Snapshot>,
    ) -> Result<SlaveView, Error> {
        self.execute_with_permit(command, snapshot, None)
    }
    /// 使用本次操作能力凭证等待结果；调用等待超时不释放仍在回调中的凭证。
    /// # Errors
    /// 命令／凭证非法、入队拒绝、真实执行失败或等待结果超时。
    pub fn execute_with_permit(
        &self,
        command: SlaveCommand,
        snapshot: Option<Snapshot>,
        parent: Option<&OperationPermit>,
    ) -> Result<SlaveView, Error> {
        self.submit_execute(command, snapshot, parent)?
            .wait_timeout(Duration::from_secs(10))?
            .ok_or_else(timeout_error)?
    }
    /// 提交一次操作并保留可继续等待的真实完成句柄；不自动重发。
    /// # Errors
    /// 命令非法、操作门占用或队列拒绝。
    pub fn execute_pending(
        &self,
        command: SlaveCommand,
        snapshot: Option<Snapshot>,
    ) -> Result<SlaveCompletion, Error> {
        self.submit_execute(command, snapshot, None)
    }
    /// 在公共操作凭证内提交，保留真实结果句柄供批量执行器继续等待。
    /// # Errors
    /// 命令或凭证非法、未应用或队列拒绝。
    pub fn execute_pending_permitted(
        &self,
        command: SlaveCommand,
        snapshot: Option<Snapshot>,
        parent: &OperationPermit,
    ) -> Result<SlaveCompletion, Error> {
        self.submit_execute(command, snapshot, Some(parent))
    }
    fn submit_execute(
        &self,
        command: SlaveCommand,
        snapshot: Option<Snapshot>,
        parent: Option<&OperationPermit>,
    ) -> Result<SlaveCompletion, Error> {
        if command.command_id.trim().is_empty() {
            return Err(Error::new("INVALID_COMMAND", "运行命令身份不能为空"));
        }
        if serde_json::to_vec(&command).map_err(failure)?.len() > 8 * 1024 * 1024 {
            return Err(Error::new("COMMAND_TOO_LARGE", "单次运行命令不得超过8MiB"));
        }
        let id = command.endpoint.clone();
        if snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.endpoint.id != id)
        {
            return Err(Error::new("ENDPOINT_MISMATCH", "启动快照不属于命令目标"));
        }
        let request = match &command.action {
            SlaveAction::Start { max_clients } => Request::Start(
                snapshot.ok_or_else(|| Error::new("NOT_APPLIED", "请先应用模拟器"))?,
                *max_clients,
                command.command_id,
            ),
            SlaveAction::Stop => Request::Stop(id.clone(), command.command_id),
            _ => Request::Values(command),
        };
        Ok(SlaveCompletion {
            receiver: self.submit(&id, request, parent)?,
            finished: false,
        })
    }
    /// 轻量运行查询，点值必须按需指定且最多1024点。
    /// # Errors
    /// 未知点、过大查询或线程不可用。
    pub fn query(&self, query: SlaveQuery) -> Result<SlaveView, Error> {
        worker::cached_query(&self.cache, query)
    }
    /// 配置成功替换或删除后释放旧静止映射，防止旧模型继续接收置数。
    /// # Errors
    /// 端点仍持有监听或所有者不可用。
    pub fn forget(&self, id: &EndpointId) -> Result<(), Error> {
        self.forget_with_permit(id, None)
    }
    /// 同一应用操作内释放旧映射，保留父操作门所有权。
    /// # Errors
    /// 凭证不匹配、端点仍运行或释放未确认。
    pub fn forget_with_permit(
        &self,
        id: &EndpointId,
        parent: Option<&OperationPermit>,
    ) -> Result<(), Error> {
        self.submit(id, Request::Forget(id.clone()), parent)?
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| timeout_error())?
            .map(|_| {
                if let Ok(mut cache) = self.cache.lock() {
                    cache.remove(id);
                }
            })
    }

    /// 取得静止运行值，供配置应用／删除边界迁移，不在高频查询使用。
    /// # Errors
    /// 端点仍在运行或线程不可用。
    pub fn stopped_values(
        &self,
        id: &EndpointId,
    ) -> Result<Option<BTreeMap<UnitId, UnitValues>>, Error> {
        match self.send(id, Request::Snapshot(id.clone()))? {
            Reply::Snapshot(values) => Ok(values),
            _ => unreachable!(),
        }
    }
}
impl Drop for TcpSlaveRuntime {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod completion_tests {
    use super::*;
    #[test]
    fn timeout_keeps_callback_gate_and_the_same_handle_receives_actual_error() {
        let gates = OperationGates::default();
        let id = EndpointId("a".into());
        let permit = gates.acquire(&id).unwrap();
        let (tx, rx) = mpsc::sync_channel(1);
        let (mut completion, tx) = (
            SlaveCompletion {
                receiver: rx,
                finished: false,
            },
            tx,
        );
        assert!(completion.wait_timeout(Duration::ZERO).unwrap().is_none());
        assert_eq!(gates.acquire(&id).err().unwrap().code, "ENDPOINT_BUSY");
        assert!(gates.ensure_available(&EndpointId("b".into())).is_ok());
        drop(permit);
        tx.send(Err(Error::new(
            "ACTUAL_BIND_FAILED",
            "真实回调确认绑定失败",
        )))
        .unwrap();
        assert_eq!(
            completion
                .wait_timeout(Duration::from_secs(1))
                .unwrap()
                .unwrap()
                .unwrap_err()
                .code,
            "ACTUAL_BIND_FAILED"
        );
        assert_eq!(
            completion.wait_timeout(Duration::ZERO).unwrap_err().code,
            "COMPLETION_CONSUMED"
        );
        assert!(gates.acquire(&id).is_ok());
    }
    #[test]
    fn lost_reply_is_unconfirmed_and_does_not_release_a_live_callback() {
        let gates = OperationGates::default();
        let id = EndpointId("a".into());
        let permit = gates.acquire(&id).unwrap();
        let (tx, rx) = mpsc::sync_channel(1);
        let mut completion = SlaveCompletion {
            receiver: rx,
            finished: false,
        };
        drop(tx);
        assert_eq!(
            completion.wait_timeout(Duration::ZERO).unwrap_err().code,
            "COMPLETION_LOST"
        );
        assert!(gates.acquire(&id).is_err());
        drop(permit);
        assert!(gates.acquire(&id).is_ok());
    }
}
