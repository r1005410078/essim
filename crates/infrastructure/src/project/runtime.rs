//! 可靠 JSONL 的异步 SQLite 投影；数据库故障不反馈为协议故障。
use super::storage;
use essim_core::domain::error::Error;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
const QUEUE_BYTES: usize = 8 * 1024 * 1024;
const LINE_BYTES: usize = 32 * 1024 * 1024;
/// 当前执行数据投影的有界队列与故障摘要。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDataView {
    /// 正在排队的投影字节数，不包含原有可靠录制队列。
    pub queued_bytes: usize,
    /// 本次工程打开期间的投影队列字节峰值。
    pub peak_queued_bytes: usize,
    /// 本轮出现投影故障的运行数。
    pub failed_runs: usize,
    /// 最近错误，不意味着协议请求失败。
    pub error: Option<String>,
}
#[derive(Default)]
struct State {
    queued: AtomicUsize,
    peak: AtomicUsize,
    failed_runs: AtomicUsize,
    error: Mutex<Option<String>>,
}
struct Core {
    tx: SyncSender<Message>,
    state: Arc<State>,
    join: Mutex<Option<JoinHandle<()>>>,
}
/// 单线程 SQLite 投影器；线程内独占数据库句柄。
#[derive(Clone)]
pub(crate) struct RunRecorder(Arc<Core>);
pub(crate) struct RunWriter {
    recorder: RunRecorder,
    id: String,
    dir: PathBuf,
    failed: Arc<AtomicBool>,
}
enum Message {
    Bytes {
        dir: PathBuf,
        id: String,
        bytes: Vec<u8>,
        failed: Arc<AtomicBool>,
    },
    Finish {
        dir: PathBuf,
        id: String,
        failed: Arc<AtomicBool>,
    },
    Flush(SyncSender<()>),
    Shutdown,
}
impl RunRecorder {
    pub(crate) fn new() -> Result<Self, Error> {
        let (tx, rx) = mpsc::sync_channel(128);
        let state = Arc::new(State::default());
        let status = state.clone();
        let join = thread::Builder::new()
            .name("project-sqlite".into())
            .spawn(move || worker(rx, status))
            .map_err(storage::io)?;
        Ok(Self(Arc::new(Core {
            tx,
            state,
            join: Mutex::new(Some(join)),
        })))
    }
    pub(crate) fn open(&self, root: &std::path::Path, id: &str) -> RunWriter {
        RunWriter {
            recorder: self.clone(),
            id: id.into(),
            dir: root.join("runs").join(id),
            failed: Arc::new(AtomicBool::new(false)),
        }
    }
    pub(crate) fn view(&self) -> RuntimeDataView {
        RuntimeDataView {
            queued_bytes: self.0.state.queued.load(Ordering::Relaxed),
            peak_queued_bytes: self.0.state.peak.load(Ordering::Relaxed),
            failed_runs: self.0.state.failed_runs.load(Ordering::Relaxed),
            error: self.0.state.error.lock().ok().and_then(|e| e.clone()),
        }
    }

    pub(crate) fn flush(&self) -> Result<(), Error> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.0
            .tx
            .try_send(Message::Flush(tx))
            .map_err(|_| Error::new("RUN_DATA_BUSY", "运行数据库尚有积压"))?;
        rx.recv_timeout(Duration::from_secs(5))
            .map_err(|_| Error::new("RUN_DATA_FLUSH", "运行数据库刷新未确认"))
    }
    fn fail(&self, id: &str, failed: &AtomicBool, message: impl Into<String>) {
        mark_failed(&self.0.state, id, failed, message.into());
    }
}
fn mark_failed(state: &State, id: &str, failed: &AtomicBool, message: String) {
    if !failed.swap(true, Ordering::AcqRel) {
        state.failed_runs.fetch_add(1, Ordering::Relaxed);
    }
    if let Ok(mut error) = state.error.lock() {
        *error = Some(format!("{id}：{message}"));
    }
}

impl RunWriter {
    pub(crate) fn path(&self) -> PathBuf {
        self.dir.join("recordings/protocol.jsonl")
    }
    pub(crate) fn append(&self, bytes: &[u8]) {
        if self.failed.load(Ordering::Acquire) {
            return;
        }
        let state = &self.recorder.0.state;
        let reservation = state
            .queued
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                n.checked_add(bytes.len()).filter(|n| *n <= QUEUE_BYTES)
            });
        let Ok(previous) = reservation else {
            self.recorder
                .fail(&self.id, &self.failed, "运行数据库队列已满，记录不完整");
            return;
        };
        state
            .peak
            .fetch_max(previous + bytes.len(), Ordering::Relaxed);
        let len = bytes.len();
        let msg = Message::Bytes {
            dir: self.dir.clone(),
            id: self.id.clone(),
            bytes: bytes.to_vec(),
            failed: self.failed.clone(),
        };
        if self.recorder.0.tx.try_send(msg).is_err() {
            state.queued.fetch_sub(len, Ordering::AcqRel);
            self.recorder
                .fail(&self.id, &self.failed, "运行数据库队列拒绝，记录不完整");
        }
    }
}
impl Drop for RunWriter {
    fn drop(&mut self) {
        if self
            .recorder
            .0
            .tx
            .try_send(Message::Finish {
                dir: self.dir.clone(),
                id: self.id.clone(),
                failed: self.failed.clone(),
            })
            .is_err()
        {
            self.recorder
                .fail(&self.id, &self.failed, "运行结束记录未入队，记录不完整");
        }
    }
}
impl Drop for Core {
    fn drop(&mut self) {
        // 不在析构无限等待磁盘。显式生命周期先调用 flush，超时由调用者报告。
        let (tx, rx) = mpsc::sync_channel(1);
        let _ = self.tx.try_send(Message::Flush(tx));
        let _ = rx.recv_timeout(Duration::from_secs(5));
        let _ = self.tx.try_send(Message::Shutdown);
        if let Ok(join) = self.join.get_mut() {
            let until = std::time::Instant::now() + Duration::from_millis(100);
            while join.as_ref().is_some_and(|j| !j.is_finished())
                && std::time::Instant::now() < until
            {
                std::thread::sleep(Duration::from_millis(1));
            }
            if join.as_ref().is_some_and(|j| j.is_finished()) {
                let _ = join.take().unwrap().join();
            }
        }
    }
}
struct Run {
    buffer: Vec<u8>,
    endpoint: String,
    sequence: i64,
    ended: bool,
    initialized: bool,
    dir: PathBuf,
    failed: Arc<AtomicBool>,
}
fn worker(rx: Receiver<Message>, state: Arc<State>) {
    let mut runs: BTreeMap<String, Run> = BTreeMap::new();
    while let Ok(message) = rx.recv() {
        match message {
            Message::Shutdown => break,
            Message::Flush(reply) => {
                runs.retain(|id, run| {
                    if run.failed.load(Ordering::Acquire) {
                        let _ = mark_incomplete(&run.dir, id);
                        false
                    } else {
                        true
                    }
                });
                let _ = reply.send(());
            }
            Message::Bytes {
                dir,
                id,
                bytes,
                failed,
            } => {
                let size = bytes.len();
                let total_buffer: usize = runs.values().map(|r| r.buffer.len()).sum();
                if runs.len() >= 4096 && !runs.contains_key(&id) {
                    mark_failed(&state, &id, &failed, "活动运行投影数量超过 4096".into());
                    state.queued.fetch_sub(size, Ordering::AcqRel);
                    let _ = mark_incomplete(&dir, &id);
                    continue;
                }
                let run = runs.entry(id.clone()).or_insert(Run {
                    buffer: vec![],
                    endpoint: String::new(),
                    sequence: 0,
                    ended: false,
                    initialized: false,
                    dir: dir.clone(),
                    failed: failed.clone(),
                });
                let result = if failed.load(Ordering::Acquire) {
                    Ok(())
                } else if total_buffer.saturating_add(bytes.len()) > LINE_BYTES {
                    Err(Error::new(
                        "RUN_DATA_LIMIT",
                        "运行快照聚合暂存超过 32 MiB，数据库投影不完整",
                    ))
                } else {
                    consume(&dir, &id, run, &bytes)
                };
                state.queued.fetch_sub(size, Ordering::AcqRel);
                if let Err(e) = result {
                    mark_failed(&state, &id, &failed, e.to_string());
                    let _ = mark_incomplete(&dir, &id);
                    run.buffer.clear();
                }
            }
            Message::Finish { dir, id, failed } => {
                if let Some(run) = runs.remove(&id) {
                    if failed.load(Ordering::Acquire) || !run.buffer.is_empty() || !run.ended {
                        let _ = mark_incomplete(&dir, &id);
                    } else {
                        if let Err(e) = update_context(&dir, &id, true, true) {
                            mark_failed(&state, &id, &failed, e.to_string());
                        }
                    }
                }
            }
        }
    }
}
fn database(dir: &std::path::Path) -> Result<Connection, Error> {
    std::fs::create_dir_all(dir).map_err(storage::io)?;
    let c = Connection::open(dir.join("runtime.sqlite")).map_err(storage::io)?;
    c.busy_timeout(Duration::from_secs(2))
        .map_err(storage::io)?;
    c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS events(sequence INTEGER PRIMARY KEY, source_sequence TEXT, unix_ms TEXT, endpoint TEXT NOT NULL, unit TEXT, kind TEXT NOT NULL, payload TEXT NOT NULL);").map_err(storage::io)?;
    Ok(c)
}
fn update_context(
    dir: &std::path::Path,
    id: &str,
    ended: bool,
    complete: bool,
) -> Result<(), Error> {
    let path = dir.join("context.json");
    let mut context: Value = if path.exists() {
        storage::read_json(&path)?
    } else {
        json!({"runId":id})
    };
    context["ended"] = json!(ended);
    context["complete"] = json!(complete);
    storage::atomic_local(&path, &storage::json(&context)?)
}
fn mark_incomplete(dir: &std::path::Path, id: &str) -> Result<(), Error> {
    update_context(dir, id, false, false)
}

fn consume(dir: &std::path::Path, id: &str, run: &mut Run, bytes: &[u8]) -> Result<(), Error> {
    if run.buffer.len() + bytes.len() > LINE_BYTES {
        return Err(Error::new(
            "RUN_DATA_LIMIT",
            "运行快照或事件超过 32 MiB 投影限制，原始通信文件仍保留",
        ));
    }
    run.buffer.extend_from_slice(bytes);
    let end = run.buffer.iter().rposition(|b| *b == b'\n');
    let Some(end) = end else {
        return Ok(());
    };
    let remainder = run.buffer.split_off(end + 1);
    let complete = std::mem::replace(&mut run.buffer, remainder);
    let mut c = database(dir)?;
    let tx = c.transaction().map_err(storage::io)?;
    for line in complete.split(|b| *b == b'\n').filter(|b| !b.is_empty()) {
        let value: Value = serde_json::from_slice(line).map_err(storage::io)?;
        if !run.initialized {
            run.endpoint = value
                .pointer("/endpoint/id")
                .or_else(|| value.pointer("/snapshot/endpoint/id"))
                .and_then(Value::as_str)
                .ok_or_else(|| Error::new("RUN_DATA_SNAPSHOT", "运行首条记录缺少端点快照"))?
                .into();
            storage::atomic_local(&dir.join("applied.json"), line)?;
            storage::atomic_local(
                &dir.join("context.json"),
                &storage::json(
                    &json!({"runId":id,"endpoint":run.endpoint,"createdUnixMs":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis().to_string(),"ended":false,"complete":false}),
                )?,
            )?;
            run.initialized = true;
            continue;
        }
        let fact = &value["fact"];
        let kind = fact["kind"]
            .as_str()
            .ok_or_else(|| Error::new("RUN_DATA_EVENT", "运行事件类型缺失"))?;
        if matches!(
            kind,
            "request" | "responsePrepared" | "responseSent" | "frame" | "rtuFrame"
        ) {
            continue;
        }
        run.sequence = run
            .sequence
            .checked_add(1)
            .ok_or_else(|| Error::new("RUN_DATA_SEQUENCE", "运行事件序号耗尽"))?;
        let unit = fact
            .get("unit")
            .and_then(Value::as_str)
            .or_else(|| fact.pointer("/command/action/unit").and_then(Value::as_str));
        tx.execute("INSERT INTO events(sequence,source_sequence,unix_ms,endpoint,unit,kind,payload) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![run.sequence,value["sequence"].as_str(),value["unixMs"].as_str(),run.endpoint,unit,kind,serde_json::to_string(&value).map_err(storage::io)?]).map_err(storage::io)?;
        if kind == "stopped" || (kind == "state" && fact["state"] == "disconnected") {
            run.ended = true;
        }
    }
    tx.commit().map_err(storage::io)?;
    if run.ended && run.buffer.is_empty() {
        update_context(dir, id, true, true)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_projection_queue_is_visible_and_does_not_wait_for_database() {
        let recorder = RunRecorder::new().unwrap();
        let writer = recorder.open(&std::env::temp_dir(), "queue-limit-check");
        recorder
            .0
            .state
            .queued
            .store(QUEUE_BYTES, Ordering::Release);
        let start = std::time::Instant::now();
        writer.append(b"data");
        assert!(start.elapsed() < Duration::from_secs(1));
        assert_eq!(recorder.view().failed_runs, 1);
        assert!(recorder.view().error.unwrap().contains("队列已满"));
        recorder.0.state.queued.store(0, Ordering::Release);
    }
}
