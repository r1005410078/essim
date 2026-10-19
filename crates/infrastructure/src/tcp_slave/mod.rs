//! TCP从站适配器：固定工作线程、线程内C句柄、独立运行查询与可靠文件记录。
mod journal;
mod mapping;
mod worker;
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
pub(super) fn failure(error: impl std::fmt::Display) -> Error {
    Error::new("TCP_RUNTIME_FAILED", error.to_string())
}
pub(super) enum Request {
    Start(Snapshot, usize, String),
    Stop(EndpointId, String),
    Query(SlaveQuery),
    Values(SlaveCommand),
    Snapshot(EndpointId),
    Forget(EndpointId),
}
pub(super) enum Reply {
    View(SlaveView),
    Snapshot(Option<BTreeMap<UnitId, UnitValues>>),
}
pub(super) struct Message {
    request: Request,
    reply: SyncSender<Result<Reply, Error>>,
}
struct Worker {
    tx: SyncSender<Message>,
    wake: Arc<Waker>,
    join: Mutex<Option<JoinHandle<()>>>,
}
/// 固定工作线程的运行服务；网络及C资源均在所属工作线程内访问。
pub struct TcpSlaveRuntime {
    workers: Vec<Worker>,
    shutdown: Arc<std::sync::atomic::AtomicBool>,
    gates: Mutex<BTreeMap<EndpointId, Arc<Mutex<()>>>>,
}
impl TcpSlaveRuntime {
    /// 创建最多4个工作线程，实际端点启动前不占用外部监听端口。
    /// # Errors
    /// poll、唤醒器或线程创建失败。
    pub fn new(root: PathBuf) -> Result<Self, Error> {
        let count = std::thread::available_parallelism()
            .map_or(1, |v| v.get())
            .min(4);
        let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut runtime = Self {
            workers: Vec::new(),
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
            let join = std::thread::Builder::new()
                .name(format!("modbus-tcp-{index}"))
                .spawn(move || worker::run(poll, rx, root, done, clients, ready))
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
    fn send(&self, id: &EndpointId, request: Request) -> Result<Reply, Error> {
        let index = id.0.bytes().fold(0usize, |v, b| {
            v.wrapping_mul(31).wrapping_add(usize::from(b))
        }) % self.workers.len();
        let worker = &self.workers[index];
        let (tx, rx) = mpsc::sync_channel(1);
        worker
            .tx
            .try_send(Message { request, reply: tx })
            .map_err(|_| Error::new("RUNTIME_BUSY", "运行命令队列已满或执行器不可用"))?;
        worker.wake.wake().map_err(failure)?;
        rx.recv_timeout(Duration::from_secs(10)).map_err(|_| {
            Error::new(
                "RUNTIME_TIMEOUT",
                "运行操作未在时限内确认；请查询实际状态，不能推断失败或重新发送写入",
            )
        })?
    }
    /// 在所有者线程启动快照或处理其他运行操作。
    /// # Errors
    /// 参数非法、记录／监听失败、队列过载或操作未确认。
    pub fn execute(
        &self,
        command: SlaveCommand,
        snapshot: Option<Snapshot>,
    ) -> Result<SlaveView, Error> {
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
        match self.send(&id, request)? {
            Reply::View(view) => Ok(view),
            _ => unreachable!(),
        }
    }
    /// 轻量运行查询，点值必须按需指定且最多1024点。
    /// # Errors
    /// 未知点、过大查询或线程不可用。
    pub fn query(&self, query: SlaveQuery) -> Result<SlaveView, Error> {
        let id = query.endpoint.clone();
        match self.send(&id, Request::Query(query))? {
            Reply::View(view) => Ok(view),
            _ => unreachable!(),
        }
    }
    /// 配置成功替换或删除后释放旧静止映射，防止旧模型继续接收置数。
    /// # Errors
    /// 端点仍持有监听或所有者不可用。
    pub fn forget(&self, id: &EndpointId) -> Result<(), Error> {
        self.send(id, Request::Forget(id.clone())).map(|_| ())
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
