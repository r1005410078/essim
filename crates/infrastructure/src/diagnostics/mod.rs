//! 有界进程诊断：业务线程仅编码并尝试入队，文件与轮转由独立线程拥有。
mod early;
mod record;
pub use early::{early_error, install_early_panic_hook};
mod sink;
mod subscriber;
pub use record::{Filter, FrontendEvent, Level};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};
/// 单条最终 JSONL 编码的字节上限。
pub const MAX_RECORD: usize = 16 * 1024;
/// 整条记录队列条数上限；与单条上限共同限制载荷为 32 MiB。
pub const QUEUE_CAPACITY: usize = 2048;
/// 与业务状态独立的诊断状态快照。
#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// 最近写入是否成功。
    pub file_available: bool,
    /// 由宿主解析的固定系统目录。
    pub directory: String,
    /// 编码队列拒收的累计条数。
    pub dropped: u64,
    /// 初始化、写入、清理与刷新失败的累计次数。
    pub write_failures: u64,
    /// 最近受限错误摘要。
    pub last_error: Option<String>,
}
/// 仅允许稳定身份及错误码的关联上下文。
#[derive(Default)]
pub struct EventContext<'a> {
    /// 调用方稳定操作身份。
    pub operation_id: Option<&'a str>,
    /// 工程稳定身份。
    pub project_id: Option<&'a str>,
    /// 端点稳定身份。
    pub endpoint_id: Option<&'a str>,
    /// 单元稳定身份。
    pub unit_id: Option<&'a str>,
    /// 机器可识别错误码。
    pub error_code: Option<&'a str>,
}
struct Shared {
    available: AtomicBool,
    dropped: AtomicU64,
    failures: AtomicU64,
    error: Mutex<Option<String>>,
    last_stderr: Mutex<Option<Instant>>,
}
impl Shared {
    fn failure(&self, error: impl std::fmt::Display) {
        self.available.store(false, Ordering::Relaxed);
        self.failures.fetch_add(1, Ordering::Relaxed);
        let text = record::clean(&error.to_string(), 512);
        if let Ok(mut last) = self.error.try_lock() {
            *last = Some(text.clone());
        }
        if let Ok(mut last) = self.last_stderr.try_lock() {
            if last.is_none_or(|t| t.elapsed() >= Duration::from_secs(30)) {
                eprintln!("ESSim 应用诊断降级：{text}");
                *last = Some(Instant::now());
            }
        }
    }
}
#[derive(Clone)]
/// 可克隆的非阻塞诊断句柄；不持有文件，也不安装全局 subscriber。
pub struct Diagnostics {
    tx: mpsc::SyncSender<Box<[u8]>>,
    shared: Arc<Shared>,
    sequence: Arc<AtomicU64>,
    directory: PathBuf,
    session: String,
    version: String,
    filter: Filter,
}
/// writer 所有者，通过显式限时 shutdown 刷新，不在 Drop 阻塞。
pub struct WriterGuard {
    stop: mpsc::Sender<mpsc::Sender<bool>>,
    shared: Arc<Shared>,
}
impl Drop for WriterGuard {
    fn drop(&mut self) {
        let (ack, _) = mpsc::channel();
        let _ = self.stop.send(ack);
    }
}
impl WriterGuard {
    /// 排空已接收记录并刷新，等待不超过 timeout；超时不会无限阻止退出。
    pub fn shutdown(&self, timeout: Duration) -> bool {
        let (tx, rx) = mpsc::channel();
        if self.stop.send(tx).is_err() {
            return self.shared.available.load(Ordering::Relaxed);
        }
        match rx.recv_timeout(timeout) {
            Ok(true) => true,
            Ok(false) => {
                self.shared.failure("日志不可写，退出队列未能落盘");
                false
            }
            Err(_) => {
                self.shared.failure("应用日志刷新超时");
                false
            }
        }
    }
}
impl Diagnostics {
    /// 路径解析失败时提供仅状态和 stderr 的降级句柄，不写入替代目录。
    pub fn unavailable(error: &str) -> (Self, WriterGuard) {
        let (tx, _rx) = mpsc::sync_channel(1);
        let (stop, _stop_rx) = mpsc::channel();
        let shared = Arc::new(Shared {
            available: AtomicBool::new(false),
            dropped: AtomicU64::new(0),
            failures: AtomicU64::new(0),
            error: Mutex::new(None),
            last_stderr: Mutex::new(None),
        });
        shared.failure(error);
        (
            Self {
                tx,
                shared: shared.clone(),
                sequence: Arc::new(AtomicU64::new(0)),
                directory: PathBuf::new(),
                session: String::new(),
                version: String::new(),
                filter: Filter::parse(None, cfg!(debug_assertions)).0,
            },
            WriterGuard { stop, shared },
        )
    }
    /// 启动独立 writer；即使目录不可写也返回可查询的降级句柄，并低频重试。
    pub fn start(directory: PathBuf, version: &str) -> (Self, WriterGuard) {
        Self::start_with(directory, version, sink::Limits::default(), QUEUE_CAPACITY)
    }
    fn start_with(
        directory: PathBuf,
        version: &str,
        limits: sink::Limits,
        capacity: usize,
    ) -> (Self, WriterGuard) {
        let (tx, rx) = mpsc::sync_channel(capacity);
        let (stop, stop_rx) = mpsc::channel();
        let shared = Arc::new(Shared {
            available: AtomicBool::new(false),
            dropped: AtomicU64::new(0),
            failures: AtomicU64::new(0),
            error: Mutex::new(None),
            last_stderr: Mutex::new(None),
        });
        let session = format!(
            "{}-{}-{:x}",
            chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ"),
            std::process::id(),
            std::collections::hash_map::RandomState::new()
                .hash_one(chrono::Utc::now().timestamp_nanos_opt())
        );
        let (filter, invalid) = Filter::parse(
            std::env::var("ESSIM_LOG").ok().as_deref(),
            cfg!(debug_assertions),
        );
        let log = Self {
            tx,
            shared: shared.clone(),
            sequence: Arc::new(AtomicU64::new(0)),
            directory,
            session,
            version: record::clip(version, 128),
            filter,
        };
        let worker = log.clone();
        if let Err(e) = std::thread::Builder::new()
            .name("application-log".into())
            .spawn(move || sink::run(worker, rx, stop_rx, limits))
        {
            shared.failure(e);
        }
        if invalid {
            log.event(
                Level::Warn,
                "essim",
                "invalid_log_filter",
                "ESSIM_LOG 非法，已回退默认过滤",
                None,
            );
        }
        (log, WriterGuard { stop, shared })
    }
    /// 取得独立诊断状态，不参与工程可靠保存的成功判定。
    pub fn status(&self) -> Status {
        Status {
            file_available: self.shared.available.load(Ordering::Relaxed),
            directory: self.directory.display().to_string(),
            dropped: self.shared.dropped.load(Ordering::Relaxed),
            write_failures: self.shared.failures.load(Ordering::Relaxed),
            last_error: self.shared.error.lock().ok().and_then(|e| e.clone()),
        }
    }
    /// 固定目录；仅供宿主打开，不接受前端路径。
    pub fn directory(&self) -> &std::path::Path {
        &self.directory
    }
    /// 记录受限文本摘要；上下文为稳定 ID，不传配置、点值或原始报文。
    pub fn event(
        &self,
        level: Level,
        target: &str,
        event: &str,
        message: &str,
        stack: Option<&str>,
    ) {
        if !self.filter.enabled(target, level) {
            return;
        }
        let bytes = self.encode_context(
            level,
            target,
            event,
            message,
            stack,
            &EventContext::default(),
        );
        if self.tx.try_send(bytes).is_err() {
            self.shared.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
    /// 记录含稳定关联身份的操作摘要，不接收完整业务对象。
    pub fn operation(&self, level: Level, event: &str, message: &str, context: &EventContext<'_>) {
        if !self.filter.enabled("essim", level) {
            return;
        }
        let bytes = self.encode_context(level, "essim", event, message, None, context);
        if self.tx.try_send(bytes).is_err() {
            self.shared.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
    fn encode(
        &self,
        level: Level,
        target: &str,
        event: &str,
        message: &str,
        stack: Option<&str>,
    ) -> Box<[u8]> {
        self.encode_context(
            level,
            target,
            event,
            message,
            stack,
            &EventContext::default(),
        )
    }
    fn encode_context(
        &self,
        level: Level,
        target: &str,
        event: &str,
        message: &str,
        stack: Option<&str>,
        context: &EventContext<'_>,
    ) -> Box<[u8]> {
        let thread = std::thread::current();
        let mut value = serde_json::json!({ "time": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true), "level":level,
            "target":record::clean(target,128), "event":record::clean(event,128), "message":record::clean(message,8192), "stack":stack.map(|s| record::clean(s,8192)),
            "appVersion":self.version, "sessionId":self.session, "pid":std::process::id(),
            "thread":record::clip(thread.name().unwrap_or("unnamed"),128), "threadId":format!("{:?}",thread.id()), "sequence":self.sequence.fetch_add(1,Ordering::Relaxed)+1,
            "truncated":message.len()>8192 || stack.is_some_and(|s| s.len()>8192) });
        for (key, id) in [
            ("operationId", context.operation_id),
            ("projectId", context.project_id),
            ("endpointId", context.endpoint_id),
            ("unitId", context.unit_id),
            ("errorCode", context.error_code),
        ] {
            if let Some(id) = id {
                value[key] = record::clean(id, 128).into();
            }
        }
        loop {
            let mut bytes = serde_json::to_vec(&value).unwrap_or_default();
            bytes.push(b'\n');
            if bytes.len() <= MAX_RECORD {
                return bytes.into_boxed_slice();
            }
            value["truncated"] = true.into();
            for field in ["message", "stack"] {
                if let Some(s) = value[field].as_str() {
                    value[field] = record::clip(s, s.len() / 2).into();
                }
            }
        }
    }
    /// 显式安装唯一的 tracing subscriber 与单向 log 桥。
    /// # Errors
    /// 宿主已经安装全局 subscriber 或 log logger 时返回错误，不替换已有 logger。
    pub fn install(&self) -> Result<(), String> {
        subscriber::install(self.clone())
    }
    /// 安装保留原始行为的 panic hook，堆栈与文本仍受整条编码限制。
    pub fn install_panic_hook(&self) {
        install_early_panic_hook();
        early::attach(self);
    }
}
use std::hash::BuildHasher;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encoded_json_is_bounded_even_with_escaped_controls() {
        let (log, guard) = Diagnostics::start(
            std::env::temp_dir().join(format!("essim-log-{}", std::process::id())),
            "test",
        );
        let bytes = log.encode(
            Level::Error,
            "essim",
            "exception",
            &"\0中文\"".repeat(10000),
            Some(&"\0".repeat(50000)),
        );
        assert!(bytes.len() <= MAX_RECORD);
        assert!(bytes.ends_with(b"\n"));
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["truncated"], true);
        assert!(guard.shutdown(Duration::from_secs(2)));
    }
    #[test]
    fn full_queue_returns_without_sink() {
        let (tx, _rx) = mpsc::sync_channel(2);
        let (mut log, guard) =
            Diagnostics::start(std::env::temp_dir().join("essim-queue-test"), "test");
        log.tx = tx;
        let start = Instant::now();
        for _ in 0..10000 {
            log.event(Level::Error, "essim", "full", "test", None);
        }
        assert_eq!(log.status().dropped, 9998);
        assert!(start.elapsed() < Duration::from_secs(2));
        assert!(guard.shutdown(Duration::from_secs(2)));
    }
}

#[cfg(test)]
mod process_tests {
    use super::*;
    #[test]
    fn subprocess_probe() {
        let Ok(mode) = std::env::var("ESSIM_DIAGNOSTIC_PROBE") else {
            return;
        };
        let directory = PathBuf::from(std::env::var_os("ESSIM_DIAGNOSTIC_ROOT").unwrap());
        install_early_panic_hook();
        early_error("before setup");
        let (log, guard) = Diagnostics::start_with(
            directory.clone(),
            "subprocess-test",
            sink::Limits {
                file: if mode == "parallel" {
                    2048
                } else {
                    1024 * 1024
                },
                history: 8192,
            },
            QUEUE_CAPACITY,
        );
        log.install().unwrap();
        log.install_panic_hook();
        match mode.as_str() {
            "worker" => {
                assert!(std::thread::Builder::new()
                    .name("panic-probe".into())
                    .spawn(|| panic!("worker probe"))
                    .unwrap()
                    .join()
                    .is_err());
            }
            "main" => {
                let failure = std::panic::catch_unwind(|| panic!("main probe"));
                guard.shutdown(Duration::from_secs(2));
                std::panic::resume_unwind(failure.unwrap_err());
            }
            "bridge" => {
                tracing::error!(event = "bridge_once", message = "one event");
                tracing_log::log::error!(target:"essim_bridge","log bridge once");
            }
            "parallel" => {
                for _ in 0..500 {
                    log.event(Level::Info, "essim", "parallel", "parallel record", None);
                }
                std::fs::write(
                    directory.join(format!("ready-{}", std::process::id())),
                    b"ready",
                )
                .unwrap();
                let deadline = Instant::now();
                while !directory.join("release").exists()
                    && deadline.elapsed() < Duration::from_secs(10)
                {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
            _ => panic!("unknown probe"),
        }
        log.event(
            Level::Info,
            "essim",
            "application_exit",
            "probe complete",
            None,
        );
        assert!(guard.shutdown(Duration::from_secs(2)));
    }
    #[test]
    fn parallel_processes_keep_active_locks_and_independent_rotation() {
        let root = std::env::temp_dir().join(format!("essim-parallel-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let mut children = vec![];
        for _ in 0..2 {
            children.push(
                std::process::Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", "diagnostics::process_tests::subprocess_probe"])
                    .env("ESSIM_DIAGNOSTIC_PROBE", "parallel")
                    .env("ESSIM_DIAGNOSTIC_ROOT", &root)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .unwrap(),
            );
        }
        let deadline = Instant::now();
        while children
            .iter()
            .any(|c| !root.join(format!("ready-{}", c.id())).exists())
            && deadline.elapsed() < Duration::from_secs(5)
        {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(children
            .iter()
            .all(|c| root.join(format!("ready-{}", c.id())).exists()));
        sink::cleanup(&root, 0).unwrap();
        assert_eq!(std::fs::read_dir(root.join("sessions")).unwrap().count(), 2);
        std::fs::write(root.join("release"), b"release").unwrap();
        for child in &mut children {
            assert!(child.wait().unwrap().success());
        }
        for session in std::fs::read_dir(root.join("sessions")).unwrap() {
            let files = std::fs::read_dir(session.unwrap().path())
                .unwrap()
                .filter_map(Result::ok)
                .filter(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .starts_with("application.jsonl")
                })
                .collect::<Vec<_>>();
            assert!(files.len() <= 10);
            for file in files {
                let body = std::fs::read_to_string(file.path()).unwrap();
                assert!(body.len() <= 2048 + MAX_RECORD);
                for line in body.lines() {
                    serde_json::from_str::<serde_json::Value>(line).unwrap();
                }
            }
        }
        sink::cleanup(&root, 0).unwrap();
        assert_eq!(std::fs::read_dir(root.join("sessions")).unwrap().count(), 0);
    }
    #[test]
    fn panic_and_early_errors_preserve_original_failure() {
        for mode in ["worker", "main", "bridge"] {
            let root =
                std::env::temp_dir().join(format!("essim-panic-{mode}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "diagnostics::process_tests::subprocess_probe",
                    "--nocapture",
                ])
                .env("ESSIM_DIAGNOSTIC_PROBE", mode)
                .env("ESSIM_DIAGNOSTIC_ROOT", &root)
                .output()
                .unwrap();
            assert_eq!(
                output.status.success(),
                mode != "main",
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let dir = std::fs::read_dir(root.join("sessions"))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            let data = std::fs::read_to_string(dir.join("application.jsonl")).unwrap();
            let records: Vec<serde_json::Value> = data
                .lines()
                .map(|l| serde_json::from_str(l).unwrap())
                .collect();
            assert!(records.iter().any(|r| r["event"] == "early_startup_error"));
            if mode == "bridge" {
                assert_eq!(
                    records
                        .iter()
                        .filter(|r| r["event"] == "bridge_once")
                        .count(),
                    1
                );
                assert_eq!(
                    records
                        .iter()
                        .filter(|r| r["message"] == "log bridge once")
                        .count(),
                    1
                );
            } else {
                assert_eq!(
                    records
                        .iter()
                        .filter(|r| r["event"] == "rust_panic")
                        .count(),
                    1
                );
                assert!(data.contains("probe"));
            }
        }
    }
}
