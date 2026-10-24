//! 固定录制线程、有界字节队列与持久化完成句柄；磁盘I/O不持有调度锁。
use crate::resources::{Reservation, ResourceManager};
use essim_core::domain::{error::Error, resources::ResourceUsage};
use std::{
    collections::{BTreeMap, VecDeque},
    io::Write,
    sync::{
        mpsc::{self, Receiver, SyncSender},
        Arc, Condvar, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
/// 可注入的可靠文件端口；成功同步才允许确认连续记录前缀。
pub trait DurableSink: Write + Send {
    /// 将已写字节可靠同步。
    /// # Errors
    /// 文件或磁盘故障。
    fn sync_data(&mut self) -> std::io::Result<()>;
}
impl DurableSink for std::fs::File {
    fn sync_data(&mut self) -> std::io::Result<()> {
        std::fs::File::sync_data(self)
    }
}
/// 一批实际可靠结束的位置。
#[derive(Debug)]
pub struct DurableReceipt {
    /// 本文件连续可靠序号。
    pub sequence: u64,
}
/// 不丢失真实后台结果的等待句柄。
pub struct RecordCompletion {
    receiver: Receiver<Result<DurableReceipt, Error>>,
}
impl RecordCompletion {
    /// None仅表示仍在队列或实际同步中，不释放额度、不推进可靠位置。
    /// # Errors
    /// 完成通道丢失，实际结果未确认。
    pub fn wait_timeout(
        &self,
        timeout: Duration,
    ) -> Result<Option<Result<DurableReceipt, Error>>, Error> {
        match self.receiver.recv_timeout(timeout) {
            Ok(v) => Ok(Some(v)),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(_) => Err(Error::new("RECORD_COMPLETION_LOST", "录制结果未确认")),
        }
    }
}
struct Entry {
    bytes: Vec<u8>,
    sequence: u64,
    reply: SyncSender<Result<DurableReceipt, Error>>,
    _charge: Reservation,
    queued: Instant,
}
struct FileQueue {
    sink: Option<Box<dyn DurableSink>>,
    projection: Option<crate::project::RunWriter>,
    queue: VecDeque<Entry>,
    bytes: usize,
    busy: bool,
    started: Option<Instant>,
    closed: bool,
    failed: Option<Error>,
    reliable: u64,
    submitted: u64,
}
#[derive(Default)]
struct State {
    files: BTreeMap<u64, FileQueue>,
    next: u64,
    ready: VecDeque<u64>,
    shutdown: bool,
    bytes: usize,
    syncs: u64,
}
/// 文件创建窄端口，生产实现延迟I/O，测试可注入慢／失败文件。
pub trait SinkFactory: Send + Sync {
    /// 构造文件所有者；实际I/O由DurableSink在录制线程执行。
    fn create(&self, path: std::path::PathBuf) -> Box<dyn DurableSink>;
}
struct Files;
impl SinkFactory for Files {
    fn create(&self, path: std::path::PathBuf) -> Box<dyn DurableSink> {
        Box::new(LazyFile::new(path))
    }
}
struct Shared {
    state: Mutex<State>,
    wake: Condvar,
    resources: ResourceManager,
    factory: Arc<dyn SinkFactory>,
    project: Mutex<Option<(std::path::PathBuf, crate::project::RunRecorder)>>,
}
struct Pool {
    shared: Arc<Shared>,
    workers: Vec<JoinHandle<()>>,
}
/// 共享4个录制执行资源，不按端点或等待超时增线程。
#[derive(Clone)]
pub struct RecordingPool {
    pool: Arc<Pool>,
}
/// 文件只有一个实际写入者；同文件后续批次不能越过前缀失败。
pub struct RecordStream {
    pool: RecordingPool,
    id: u64,
    path: Option<std::path::PathBuf>,
}
/// 有界队列状态，不等待文件或sync_data。
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingView {
    /// 尚未真实结束的字节，含正在同步的批次。
    pub queued_bytes: usize,
    /// 活动文件。
    pub files: usize,
    /// 实际完成的文件同步次数。
    pub syncs: u64,
    /// 固定录制线程数。
    pub workers: usize,
    /// 正在真实写入／同步的文件数。
    pub active_files: usize,
    /// 已拒绝后续写入的失败文件数。
    pub failed_files: usize,
    /// 最早未完成项已等待的毫秒，含实际同步。
    pub oldest_wait_ms: u64,
}
fn poisoned() -> Error {
    Error::new("STATE_POISONED", "录制调度状态不可用")
}
impl RecordingPool {
    /// 创建固定4线程录制池，失败时回收已创建线程。
    /// # Errors
    /// 系统线程创建失败。
    pub fn new(resources: ResourceManager) -> Result<Self, Error> {
        Self::with_factory(resources, Arc::new(Files))
    }
    /// 注入文件端口，保留相同固定线程和队列语义。
    /// # Errors
    /// 固定线程创建失败。
    pub fn with_factory(
        resources: ResourceManager,
        factory: Arc<dyn SinkFactory>,
    ) -> Result<Self, Error> {
        let shared = Arc::new(Shared {
            state: Default::default(),
            wake: Condvar::new(),
            resources,
            factory,
            project: Mutex::new(None),
        });
        let mut workers = Vec::new();
        for i in 0..4 {
            let s = shared.clone();
            match thread::Builder::new()
                .name(format!("essim-record-{i}"))
                .spawn(move || worker(s))
            {
                Ok(h) => workers.push(h),
                Err(e) => {
                    shared.state.lock().map_err(|_| poisoned())?.shutdown = true;
                    shared.wake.notify_all();
                    for h in workers {
                        let _ = h.join();
                    }
                    return Err(Error::new("THREAD_CREATE_FAILED", e.to_string()));
                }
            }
        }
        Ok(Self {
            pool: Arc::new(Pool { shared, workers }),
        })
    }
    /// 取得由固定录制线程实际创建的独立文件。
    /// # Errors
    /// 文件身份或池状态不可用。
    pub fn open(&self, path: std::path::PathBuf) -> Result<RecordStream, Error> {
        let projection = self
            .pool
            .shared
            .project
            .lock()
            .map_err(|_| poisoned())?
            .as_ref()
            .map(|(root, recorder)| {
                let id = path.file_stem().unwrap_or_default().to_string_lossy();
                recorder.open(root, &id)
            });
        let actual = projection.as_ref().map(|p| p.path()).unwrap_or(path);
        let mut stream = self.stream(self.pool.shared.factory.create(actual.clone()))?;
        if let Some(file) = self
            .pool
            .shared
            .state
            .lock()
            .map_err(|_| poisoned())?
            .files
            .get_mut(&stream.id)
        {
            file.projection = projection;
        }
        stream.path = Some(actual);
        Ok(stream)
    }
    /// 注册独立文件；文件创建及起始快照由调用者通过首批次写入，不与其他文件共用可靠位置。
    /// # Errors
    /// 状态不可用、身份耗尽或池已退出。
    pub fn stream(&self, sink: Box<dyn DurableSink>) -> Result<RecordStream, Error> {
        let mut s = self.pool.shared.state.lock().map_err(|_| poisoned())?;
        if s.shutdown {
            return Err(Error::new("RECORDING_STOPPING", "录制池已退出"));
        }
        s.next = s
            .next
            .checked_add(1)
            .ok_or_else(|| Error::new("RECORDING_OVERFLOW", "录制文件身份耗尽"))?;
        let id = s.next;
        s.files.insert(
            id,
            FileQueue {
                sink: Some(sink),
                projection: None,
                queue: VecDeque::new(),
                bytes: 0,
                busy: false,
                started: None,
                closed: false,
                failed: None,
                reliable: 0,
                submitted: 0,
            },
        );
        Ok(RecordStream {
            pool: self.clone(),
            id,
            path: None,
        })
    }
    /// 切换后续录制的工程根目录；现有 stream 继续属于原运行。
    /// # Errors
    /// 数据库线程创建或状态锁失败。
    pub fn set_project(&self, root: Option<std::path::PathBuf>) -> Result<(), Error> {
        let next = root
            .map(|root| crate::project::RunRecorder::new().map(|r| (root, r)))
            .transpose()?;
        *self.pool.shared.project.lock().map_err(|_| poisoned())? = next;
        Ok(())
    }
    /// 等待已入队的 SQLite 投影落盘，不影响原有可靠 JSONL 确认。
    /// # Errors
    /// 队列积压或限时刷新未确认。
    pub fn flush_project(&self) -> Result<(), Error> {
        if let Some((_, r)) = self
            .pool
            .shared
            .project
            .lock()
            .map_err(|_| poisoned())?
            .as_ref()
        {
            r.flush()?;
        }
        Ok(())
    }
    /// SQLite 投影故障摘要；不等于协议或 JSONL 失败。
    pub fn project_view(&self) -> Option<crate::project::RuntimeDataView> {
        self.pool
            .shared
            .project
            .lock()
            .ok()?
            .as_ref()
            .map(|(_, r)| r.view())
    }
    /// 查询调度状态，不等待磁盘或文件锁。
    /// # Errors
    /// 状态不可用。
    pub fn query(&self) -> Result<RecordingView, Error> {
        let s = self.pool.shared.state.lock().map_err(|_| poisoned())?;
        Ok(RecordingView {
            queued_bytes: s.bytes,
            files: s.files.len(),
            syncs: s.syncs,
            workers: self.pool.workers.len(),
            active_files: s.files.values().filter(|f| f.busy).count(),
            failed_files: s.files.values().filter(|f| f.failed.is_some()).count(),
            oldest_wait_ms: s
                .files
                .values()
                .filter_map(|f| f.started.or_else(|| f.queue.front().map(|e| e.queued)))
                .min()
                .map_or(0, |t| t.elapsed().as_millis().min(u64::MAX as u128) as u64),
        })
    }
}
impl RecordStream {
    /// 真实录制路径；注入测试 sink 时为空。
    pub fn path(&self) -> Option<&std::path::Path> {
        self.path.as_deref()
    }

    /// 提交一批已经满足因果前置条件的连续JSONL记录。
    /// 批次内不得放入依赖尚未发生动作的完成事实；后续阶段必须等待本句柄确认。
    /// # Errors
    /// 序号不连续、批次／文件／总字节超限、文件已失败或退出。失败不推进提交位置。
    pub fn append(&self, bytes: Vec<u8>, sequence: u64) -> Result<RecordCompletion, Error> {
        let policy = self.pool.pool.shared.resources.policy();
        if bytes.is_empty() || bytes.len() > policy.record_batch_bytes {
            return Err(Error::new(
                "RECORD_BATCH_LIMIT",
                "录制批次为空或超过字节上限",
            ));
        }
        let charge = self.pool.pool.shared.resources.reserve(ResourceUsage {
            queue_bytes: bytes.len() as u64,
            memory_bytes: bytes.len() as u64,
            ..Default::default()
        })?;
        let shared = &self.pool.pool.shared;
        let mut s = shared.state.lock().map_err(|_| poisoned())?;
        if s.shutdown {
            return Err(Error::new("RECORDING_STOPPING", "录制池正在退出"));
        }
        let file = s
            .files
            .get_mut(&self.id)
            .ok_or_else(|| Error::new("RECORD_CLOSED", "文件已关闭"))?;
        if let Some(e) = &file.failed {
            return Err(e.clone());
        }
        if Some(sequence) != file.submitted.checked_add(1) {
            return Err(Error::new("RECORD_SEQUENCE_GAP", "录制提交必须为连续前缀"));
        }
        if file.bytes.saturating_add(bytes.len()) > policy.endpoint_record_bytes {
            return Err(Error::new(
                "RECORD_QUEUE_FULL",
                "单端点录制队列已满，必须停止受影响实例",
            ));
        }
        let (tx, rx) = mpsc::sync_channel(1);
        let len = bytes.len();
        file.bytes += len;
        file.submitted = sequence;
        charge.commit();
        file.queue.push_back(Entry {
            bytes,
            sequence,
            reply: tx,
            _charge: charge,
            queued: Instant::now(),
        });
        let schedule = !file.busy && file.queue.len() == 1;
        if schedule {
            s.ready.push_back(self.id);
        }
        s.bytes += len;
        drop(s);
        shared.wake.notify_all();
        Ok(RecordCompletion { receiver: rx })
    }
    /// 当前文件真实可靠前缀；排队和等待超时不推进它。
    /// # Errors
    /// 文件已关闭或状态不可用。
    pub fn reliable(&self) -> Result<u64, Error> {
        self.pool
            .pool
            .shared
            .state
            .lock()
            .map_err(|_| poisoned())?
            .files
            .get(&self.id)
            .map(|f| f.reliable)
            .ok_or_else(|| Error::new("RECORD_CLOSED", "文件已关闭"))
    }
}
impl Drop for RecordStream {
    fn drop(&mut self) {
        if let Ok(mut s) = self.pool.pool.shared.state.lock() {
            if let Some(f) = s.files.get_mut(&self.id) {
                f.closed = true;
                if !f.busy && f.queue.is_empty() {
                    s.files.remove(&self.id);
                }
            }
        }
    }
}
fn worker(shared: Arc<Shared>) {
    loop {
        let (id, mut sink, batch) = {
            let Ok(mut s) = shared.state.lock() else {
                return;
            };
            loop {
                if let Some(id) = s.ready.pop_front() {
                    let Some(f) = s.files.get_mut(&id) else {
                        continue;
                    };
                    if f.busy || f.queue.is_empty() {
                        continue;
                    }
                    let wait = Duration::from_millis(shared.resources.policy().record_wait_ms);
                    let elapsed = f.queue.front().unwrap().queued.elapsed();
                    if !s.shutdown && elapsed < wait {
                        s.ready.push_back(id);
                        let Ok((next, _)) = shared.wake.wait_timeout(s, wait - elapsed) else {
                            return;
                        };
                        s = next;
                        continue;
                    }
                    let f = s.files.get_mut(&id).unwrap();
                    f.busy = true;
                    f.started = Some(Instant::now());
                    let Some(sink) = f.sink.take() else {
                        return;
                    };
                    let mut batch = Vec::new();
                    let mut bytes = 0;
                    while f.queue.front().is_some_and(|e| {
                        bytes + e.bytes.len() <= shared.resources.policy().record_batch_bytes
                    }) {
                        let e = f.queue.pop_front().unwrap();
                        bytes += e.bytes.len();
                        batch.push(e);
                    }
                    break (id, sink, batch);
                }
                if s.shutdown {
                    return;
                }
                let Ok(next) = shared.wake.wait(s) else {
                    return;
                };
                s = next;
            }
        };
        let result = (|| {
            for entry in &batch {
                sink.write_all(&entry.bytes)?;
            }
            sink.flush()?;
            sink.sync_data()
        })()
        .map_err(|e| Error::new("RECORDING_FAILED", format!("可靠录制失败：{e}")));
        let Ok(mut s) = shared.state.lock() else {
            return;
        };
        let bytes = batch.iter().map(|e| e.bytes.len()).sum::<usize>();
        s.bytes -= bytes;
        s.syncs += 1;
        let Some(file) = s.files.get_mut(&id) else {
            return;
        };
        file.bytes -= bytes;
        file.busy = false;
        file.started = None;
        file.sink = Some(sink);
        if result.is_ok() {
            if let Some(projection) = &file.projection {
                for entry in &batch {
                    projection.append(&entry.bytes);
                }
            }
            file.reliable = batch.last().unwrap().sequence;
        } else {
            file.failed = result.clone().err();
        }
        let reliable = file.reliable;
        let mut rejected = Vec::new();
        if result.is_err() {
            rejected.extend(file.queue.drain(..));
            file.bytes = 0;
        }
        let schedule = !file.queue.is_empty();
        let remove = file.closed && !file.busy && file.queue.is_empty();
        s.bytes -= rejected.iter().map(|e| e.bytes.len()).sum::<usize>();
        if schedule {
            s.ready.push_back(id);
        }
        if remove {
            s.files.remove(&id);
        }
        drop(s);
        for entry in batch {
            let _ = entry.reply.send(
                result
                    .clone()
                    .map(|_| DurableReceipt { sequence: reliable }),
            );
        }
        for entry in rejected {
            let _ = entry.reply.send(Err(result.clone().err().unwrap()));
        }
        shared.wake.notify_all();
    }
}
impl Drop for Pool {
    fn drop(&mut self) {
        if let Ok(mut s) = self.shared.state.lock() {
            s.shutdown = true;
        }
        self.shared.wake.notify_all();
        for h in self.workers.drain(..) {
            let _ = h.join();
        }
    }
}

/// 将一个JSONL值分块写入同一文件，块字节有界；全部可靠完成后才返回。
/// 仅供具有独立所有者的同步适配器使用；事件循环须直接等待异步完成句柄。
/// # Errors
/// 序列化、队列准入或真实持久化失败。
pub fn write_json(
    stream: &RecordStream,
    sequence: &mut u64,
    value: &impl serde::Serialize,
) -> Result<(), Error> {
    struct Chunks<'a> {
        stream: &'a RecordStream,
        sequence: &'a mut u64,
        bytes: Vec<u8>,
        capacity: usize,
    }
    impl Write for Chunks<'_> {
        fn write(&mut self, mut bytes: &[u8]) -> io::Result<usize> {
            let count = bytes.len();
            while !bytes.is_empty() {
                let n = (self.capacity - self.bytes.len()).min(bytes.len());
                self.bytes.extend_from_slice(&bytes[..n]);
                bytes = &bytes[n..];
                if self.bytes.len() == self.capacity {
                    self.flush()?;
                }
            }
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            if self.bytes.is_empty() {
                return Ok(());
            }
            let next = self
                .sequence
                .checked_add(1)
                .ok_or_else(|| io::Error::other("录制序号耗尽"))?;
            let bytes = std::mem::replace(&mut self.bytes, Vec::with_capacity(self.capacity));
            let completion = self
                .stream
                .append(bytes, next)
                .map_err(|e| io::Error::other(e.message))?;
            loop {
                if let Some(result) = completion
                    .wait_timeout(Duration::from_millis(100))
                    .map_err(|e| io::Error::other(e.message))?
                {
                    result.map_err(|e| io::Error::other(e.message))?;
                    *self.sequence = next;
                    return Ok(());
                }
            }
        }
    }
    use std::io;
    let capacity = stream
        .pool
        .pool
        .shared
        .resources
        .policy()
        .record_batch_bytes;
    let mut writer = Chunks {
        stream,
        sequence,
        bytes: Vec::with_capacity(capacity),
        capacity,
    };
    serde_json::to_writer(&mut writer, value)
        .map_err(|e| Error::new("RECORDING_FAILED", e.to_string()))?;
    writer
        .write_all(b"\n")
        .and_then(|_| writer.flush())
        .map_err(|e| Error::new("RECORDING_FAILED", e.to_string()))
}

/// 延迟到录制线程创建文件与目录；首次同步同时同步目录项。
pub struct LazyFile {
    path: std::path::PathBuf,
    file: Option<std::fs::File>,
    directory_synced: bool,
}
impl LazyFile {
    /// 构造不执行文件I/O；路径必须是本次新运行文件。
    pub fn new(path: std::path::PathBuf) -> Self {
        Self {
            path,
            file: None,
            directory_synced: false,
        }
    }
    fn file(&mut self) -> std::io::Result<&mut std::fs::File> {
        if self.file.is_none() {
            if let Some(root) = self.path.parent() {
                std::fs::create_dir_all(root)?;
            }
            self.file = Some(
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&self.path)?,
            );
        }
        Ok(self.file.as_mut().unwrap())
    }
}
impl Write for LazyFile {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.file()?.write(b)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file()?.flush()
    }
}
impl DurableSink for LazyFile {
    fn sync_data(&mut self) -> std::io::Result<()> {
        self.file()?.sync_data()?;
        if !self.directory_synced {
            if let Some(root) = self.path.parent() {
                std::fs::File::open(root)?.sync_all()?;
            }
            self.directory_synced = true;
        }
        Ok(())
    }
}
impl RecordCompletion {
    /// 供固定所有者的本地协作调度器轮询；调度器定期轮询，句柄不创建等待线程。
    /// # Errors
    /// 实际同步失败或通道丢失。
    pub async fn wait(&self) -> Result<DurableReceipt, Error> {
        std::future::poll_fn(|_| match self.receiver.try_recv() {
            Ok(v) => std::task::Poll::Ready(v),
            Err(mpsc::TryRecvError::Empty) => std::task::Poll::Pending,
            Err(_) => {
                std::task::Poll::Ready(Err(Error::new("RECORD_COMPLETION_LOST", "录制结果未确认")))
            }
        })
        .await
    }
}
/// 异步分块写入JSONL；只有对应文件全部块可靠确认后返回。
/// # Errors
/// 内存／队列准入、序列化或可靠写入失败。
pub async fn write_json_async(
    stream: &RecordStream,
    sequence: &mut u64,
    value: &impl serde::Serialize,
) -> Result<(), Error> {
    let bytes = crate::resources::serialized_size(value)?.saturating_add(1);
    let _staging = stream.pool.pool.shared.resources.reserve(ResourceUsage {
        memory_bytes: bytes,
        ..Default::default()
    })?;
    let mut json =
        serde_json::to_vec(value).map_err(|e| Error::new("RECORDING_FAILED", e.to_string()))?;
    json.push(b'\n');
    for chunk in json.chunks(
        stream
            .pool
            .pool
            .shared
            .resources
            .policy()
            .record_batch_bytes,
    ) {
        let next = sequence
            .checked_add(1)
            .ok_or_else(|| Error::new("RECORD_SEQUENCE_OVERFLOW", "录制序号耗尽"))?;
        stream.append(chunk.to_vec(), next)?.wait().await?;
        *sequence = next;
    }
    Ok(())
}
