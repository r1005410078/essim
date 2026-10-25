use super::*;
use rolling_file::{BasicRollingFileAppender, RollingConditionBasic};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::mpsc,
};
const MARKER: &str = "essim-application-log-v1\n";
#[derive(Clone, Copy)]
pub(super) struct Limits {
    pub file: u64,
    pub history: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            file: 10 * 1024 * 1024,
            history: 100 * 1024 * 1024,
        }
    }
}
struct Sink {
    writer: BasicRollingFileAppender,
    _lock: File,
    dir: PathBuf,
    size: u64,
    limits: Limits,
}
impl Sink {
    fn open(root: &Path, session: &str, limits: Limits) -> io::Result<Self> {
        let dir = root.join("sessions").join(session);
        fs::create_dir_all(&dir)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(dir.join("session.lock"))?;
        lock.try_lock()
            .map_err(|e| io::Error::other(e.to_string()))?;
        fs::write(dir.join("format"), MARKER)?;
        let path = dir.join("application.jsonl");
        let mut size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        if size > 0 {
            let mut file = OpenOptions::new().read(true).write(true).open(&path)?;
            let start = size.saturating_sub(MAX_RECORD as u64);
            file.seek(SeekFrom::Start(start))?;
            let mut tail = Vec::new();
            file.read_to_end(&mut tail)?;
            if tail.last() != Some(&b'\n') {
                size = start + tail.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1) as u64;
                file.set_len(size)?;
            }
        }
        let writer = BasicRollingFileAppender::new_with_buffer_capacity(
            path,
            RollingConditionBasic::new(),
            9,
            0,
        )?;
        Ok(Self {
            writer,
            _lock: lock,
            dir,
            size,
            limits,
        })
    }
    fn write(&mut self, bytes: &[u8]) -> io::Result<bool> {
        let rolled = self.size > 0 && self.size + bytes.len() as u64 > self.limits.file;
        if rolled {
            self.writer.rollover()?;
            self.size = 0;
        }
        // 无缓冲写入；失败时尽力恢复至上一条完整 JSONL 边界。
        if let Err(e) = self
            .writer
            .write_all(bytes)
            .and_then(|_| self.writer.flush())
        {
            let _ = OpenOptions::new()
                .write(true)
                .open(self.dir.join("application.jsonl"))
                .and_then(|f| f.set_len(self.size));
            return Err(e);
        }
        self.size += bytes.len() as u64;
        Ok(rolled)
    }
}
pub(super) fn cleanup(root: &Path, budget: u64) -> io::Result<()> {
    let sessions = root.join("sessions");
    if !fs::symlink_metadata(&sessions)?.file_type().is_dir() {
        return Err(io::Error::other("日志会话根目录不是普通目录"));
    }
    let mut candidates = vec![];
    for entry in fs::read_dir(root.join("sessions"))? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let dir = entry.path();
        // 不跟随未知文件或符号链接；仅识别本模块生成的平面会话目录。
        let entries = fs::read_dir(&dir)?.collect::<Result<Vec<_>, _>>()?;
        if entries
            .iter()
            .any(|e| e.file_type().map_or(true, |t| !t.is_file()))
        {
            continue;
        }
        if fs::read_to_string(dir.join("format")).ok().as_deref() != Some(MARKER) {
            continue;
        }
        if entries.iter().any(|e| {
            let n = e.file_name();
            let n = n.to_string_lossy();
            n != "format"
                && n != "session.lock"
                && n != "application.jsonl"
                && !(1..=9).any(|i| n == format!("application.jsonl.{i}"))
        }) {
            continue;
        }
        let lock = match OpenOptions::new()
            .read(true)
            .write(true)
            .open(dir.join("session.lock"))
        {
            Ok(l) => l,
            Err(_) => continue,
        };
        if lock.try_lock().is_err() {
            continue;
        }
        let bytes = entries
            .iter()
            .try_fold(0u64, |sum, e| Ok::<_, io::Error>(sum + e.metadata()?.len()))?;
        candidates.push((entry.file_name(), dir, bytes, lock));
    }
    candidates.sort_by(|a, b| a.0.cmp(&b.0));
    let mut total: u64 = candidates.iter().map(|c| c.2).sum();
    for (_, dir, bytes, _lock) in candidates {
        if total <= budget {
            break;
        }
        // 持锁删除，启动器不重用旧会话 ID。
        fs::remove_dir_all(dir)?;
        total = total.saturating_sub(bytes);
    }
    Ok(())
}
trait RecordSink: Send {
    fn write_record(&mut self, bytes: &[u8]) -> io::Result<bool>;
    fn flush(&mut self) -> io::Result<()>;
}
impl RecordSink for Sink {
    fn write_record(&mut self, bytes: &[u8]) -> io::Result<bool> {
        self.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
pub(super) fn run(
    log: Diagnostics,
    rx: mpsc::Receiver<Box<[u8]>>,
    stop: mpsc::Receiver<mpsc::Sender<bool>>,
    limits: Limits,
) {
    let root = log.directory.clone();
    let session = log.session.clone();
    run_with(log, rx, stop, limits, Duration::from_secs(30), move || {
        Sink::open(&root, &session, limits).map(|s| Box::new(s) as Box<dyn RecordSink>)
    });
}
fn run_with(
    log: Diagnostics,
    rx: mpsc::Receiver<Box<[u8]>>,
    stop: mpsc::Receiver<mpsc::Sender<bool>>,
    limits: Limits,
    retry_interval: Duration,
    mut open: impl FnMut() -> io::Result<Box<dyn RecordSink>>,
) {
    let mut sink = None;
    let mut retry = Instant::now() - retry_interval;
    let mut reported = 0;
    let mut last_summary = Instant::now() - Duration::from_secs(1);
    loop {
        let shutdown = stop.try_recv().ok();
        if sink.is_none() && retry.elapsed() >= retry_interval {
            retry = Instant::now();
            match open() {
                Ok(s) => {
                    sink = Some(s);
                    log.shared.available.store(true, Ordering::Relaxed);
                    if let Err(e) = cleanup(&log.directory, limits.history) {
                        log.shared.failure(e);
                    }
                }
                Err(e) => log.shared.failure(e),
            }
        }
        let record = if shutdown.is_some() {
            rx.try_recv().ok()
        } else {
            rx.recv_timeout(Duration::from_millis(50)).ok()
        };
        if let Some(bytes) = record {
            if let Some(s) = sink.as_mut() {
                let result = s.write_record(&bytes);
                match result {
                    Ok(rolled) => {
                        log.shared.available.store(true, Ordering::Relaxed);
                        if rolled {
                            if let Err(e) = cleanup(&log.directory, limits.history) {
                                log.shared.failure(e);
                            }
                        }
                        let dropped = log.shared.dropped.load(Ordering::Relaxed);
                        if dropped > reported && last_summary.elapsed() >= Duration::from_secs(1) {
                            let summary = log.encode(
                                Level::Warn,
                                "essim",
                                "diagnostic_dropped",
                                &format!("累计丢弃 {dropped} 条诊断"),
                                None,
                            );
                            match s.write_record(&summary) {
                                Err(e) => {
                                    log.shared.failure(e);
                                    sink = None;
                                    retry = Instant::now();
                                }
                                Ok(rolled) => {
                                    reported = dropped;
                                    last_summary = Instant::now();
                                    if rolled {
                                        if let Err(e) = cleanup(&log.directory, limits.history) {
                                            log.shared.failure(e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log.shared.failure(e);
                        log.shared.dropped.fetch_add(1, Ordering::Relaxed);
                        sink = None;
                        retry = Instant::now();
                    }
                }
            } else {
                log.shared.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
        if let Some(ack) = shutdown {
            // 停止请求独立于数据队列，即使队列已满仍能收取。
            for bytes in rx.try_iter() {
                if let Some(s) = sink.as_mut() {
                    if let Err(e) = s.write_record(&bytes) {
                        log.shared.failure(e);
                        log.shared.dropped.fetch_add(1, Ordering::Relaxed);
                        sink = None;
                    }
                } else {
                    log.shared.dropped.fetch_add(1, Ordering::Relaxed);
                }
            }
            let dropped = log.shared.dropped.load(Ordering::Relaxed);
            let flushed = if let Some(s) = sink.as_mut() {
                let result = if dropped > reported {
                    let summary = log.encode(
                        Level::Warn,
                        "essim",
                        "diagnostic_dropped",
                        &format!("累计丢弃 {dropped} 条诊断"),
                        None,
                    );
                    s.write_record(&summary).and_then(|_| s.flush())
                } else {
                    s.flush()
                };
                match result {
                    Ok(()) => true,
                    Err(e) => {
                        log.shared.failure(e);
                        false
                    }
                }
            } else {
                false
            };
            let _ = ack.send(flushed);
            return;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("essim-log-{name}-{}", std::process::id()))
    }
    #[test]
    fn rotation_retains_order_and_complete_json() {
        let root = root("rotation");
        let _ = fs::remove_dir_all(&root);
        let mut sink = Sink::open(
            &root,
            "a",
            Limits {
                file: 120,
                history: 10000,
            },
        )
        .unwrap();
        for n in 0..500 {
            sink.write(format!("{{\"sequence\":{n},\"message\":\"中文\"}}\n").as_bytes())
                .unwrap();
        }
        let mut sequence = vec![];
        for n in (0..=9).rev() {
            let name = if n == 0 {
                "application.jsonl".into()
            } else {
                format!("application.jsonl.{n}")
            };
            let data = fs::read_to_string(sink.dir.join(name)).unwrap();
            assert!(data.len() <= 120);
            for line in data.lines() {
                let v: serde_json::Value = serde_json::from_str(line).unwrap();
                sequence.push(v["sequence"].as_u64().unwrap());
            }
        }
        assert!(sequence.windows(2).all(|w| w[1] == w[0] + 1));
        assert_eq!(sequence.last(), Some(&499));
        assert!(!sink.dir.join("application.jsonl.10").exists());
    }
    #[test]
    fn retry_discards_partial_tail_before_next_json_record() {
        let root = root("partial-tail");
        let _ = fs::remove_dir_all(&root);
        let sink = Sink::open(&root, "session", Limits::default()).unwrap();
        let path = sink.dir.join("application.jsonl");
        drop(sink);
        fs::write(&path, b"{\"sequence\":1}\n{\"sequence\":2").unwrap();
        let mut recovered = Sink::open(&root, "session", Limits::default()).unwrap();
        recovered.write(b"{\"sequence\":3}\n").unwrap();
        let lines = fs::read_to_string(path).unwrap();
        assert_eq!(lines.lines().count(), 2);
        for line in lines.lines() {
            serde_json::from_str::<serde_json::Value>(line).unwrap();
        }
    }
    #[test]
    fn cleanup_preserves_active_and_foreign() {
        let root = root("cleanup");
        let _ = fs::remove_dir_all(&root);
        let old = Sink::open(&root, "a", Limits::default()).unwrap();
        drop(old);
        let active = Sink::open(&root, "b", Limits::default()).unwrap();
        fs::create_dir_all(root.join("sessions/foreign")).unwrap();
        cleanup(&root, 0).unwrap();
        assert!(!root.join("sessions/a").exists());
        assert!(active.dir.exists());
        assert!(root.join("sessions/foreign").exists());
    }
    #[test]
    fn initialization_failure_is_independent_and_shutdown_bounded() {
        let root = root("unwritable");
        let _ = fs::remove_file(&root);
        fs::write(&root, b"not a directory").unwrap();
        let (log, guard) = Diagnostics::start(root, "test");
        for _ in 0..100 {
            if log.status().write_failures > 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!log.status().file_available);
        assert!(log.status().write_failures > 0);
        log.event(
            Level::Error,
            "essim",
            "still_running",
            "operation unaffected",
            None,
        );
        assert!(!guard.shutdown(Duration::from_secs(2)));
    }
}

#[cfg(test)]
mod fault_tests {
    use super::*;
    struct Blocked {
        entered: mpsc::Sender<()>,
        release: mpsc::Receiver<()>,
    }
    impl RecordSink for Blocked {
        fn write_record(&mut self, _: &[u8]) -> io::Result<bool> {
            let _ = self.entered.send(());
            let _ = self.release.recv();
            Ok(false)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn blocked_disk_cannot_block_enqueue_or_shutdown_deadline() {
        let (mut log, old) =
            Diagnostics::start(std::env::temp_dir().join("essim-fault-detach"), "test");
        old.shutdown(Duration::from_secs(2));
        let (tx, rx) = mpsc::sync_channel(2);
        log.tx = tx;
        let (stop, stop_rx) = mpsc::channel();
        let guard = WriterGuard {
            stop,
            shared: log.shared.clone(),
        };
        let (entered, seen) = mpsc::channel();
        let (release, blocked) = mpsc::channel();
        let mut sink = Some(Blocked {
            entered,
            release: blocked,
        });
        let worker = log.clone();
        let handle = std::thread::spawn(move || {
            run_with(
                worker,
                rx,
                stop_rx,
                Limits::default(),
                Duration::from_millis(10),
                move || Ok(Box::new(sink.take().unwrap())),
            )
        });
        log.event(Level::Error, "essim", "blocked", "one", None);
        seen.recv_timeout(Duration::from_secs(2)).unwrap();
        let start = Instant::now();
        for _ in 0..1000 {
            log.event(Level::Error, "essim", "storm", "two", None);
        }
        assert!(start.elapsed() < Duration::from_secs(1));
        assert!(log.status().dropped >= 998);
        let start = Instant::now();
        assert!(!guard.shutdown(Duration::from_millis(30)));
        assert!(start.elapsed() < Duration::from_millis(250));
        // 解除测试 sink，使所有后台资源可回收；不让测试遗留阻塞线程。
        for _ in 0..8 {
            let _ = release.send(());
        }
        handle.join().unwrap();
    }
    struct Failure;
    impl RecordSink for Failure {
        fn write_record(&mut self, _: &[u8]) -> io::Result<bool> {
            Err(io::Error::from_raw_os_error(28))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn disk_full_retries_at_bounded_frequency_and_reports_independently() {
        let (mut log, old) =
            Diagnostics::start(std::env::temp_dir().join("essim-fault-full"), "test");
        old.shutdown(Duration::from_secs(2));
        let (tx, rx) = mpsc::sync_channel(2);
        log.tx = tx;
        let (stop, stop_rx) = mpsc::channel();
        let worker = log.clone();
        let opens = Arc::new(AtomicU64::new(0));
        let count = opens.clone();
        let thread = std::thread::spawn(move || {
            run_with(
                worker,
                rx,
                stop_rx,
                Limits::default(),
                Duration::from_millis(100),
                move || {
                    count.fetch_add(1, Ordering::Relaxed);
                    Ok(Box::new(Failure))
                },
            )
        });
        for _ in 0..100 {
            log.event(Level::Error, "essim", "failure", "independent", None);
        }
        std::thread::sleep(Duration::from_millis(80));
        assert!(log.status().write_failures > 0);
        assert!(!log.status().file_available);
        assert!(opens.load(Ordering::Relaxed) <= 2);
        let (ack, done) = mpsc::channel();
        stop.send(ack).unwrap();
        done.recv_timeout(Duration::from_secs(2)).unwrap();
        thread.join().unwrap();
    }
    #[test]
    fn permission_failure_recovers_without_changing_business_state() {
        let root = std::env::temp_dir().join(format!("essim-log-recovery-{}", std::process::id()));
        let (mut log, old) = Diagnostics::start(root.clone(), "test");
        old.shutdown(Duration::from_secs(2));
        let (tx, rx) = mpsc::sync_channel(8);
        log.tx = tx;
        let (stop, stop_rx) = mpsc::channel();
        let worker = log.clone();
        let session = log.session.clone();
        let thread = std::thread::spawn(move || {
            let mut attempts = 0;
            run_with(
                worker,
                rx,
                stop_rx,
                Limits::default(),
                Duration::from_millis(20),
                move || {
                    attempts += 1;
                    if attempts == 1 {
                        Err(io::Error::from(io::ErrorKind::PermissionDenied))
                    } else {
                        Sink::open(&root, &session, Limits::default())
                            .map(|s| Box::new(s) as Box<dyn RecordSink>)
                    }
                },
            )
        });
        let start = Instant::now();
        while log.status().write_failures == 0 && start.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(log.status().write_failures > 0);
        let start = Instant::now();
        while !log.status().file_available && start.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(log.status().file_available);
        log.event(Level::Info, "essim", "after_recovery", "recovered", None);
        let (ack, done) = mpsc::channel();
        stop.send(ack).unwrap();
        done.recv_timeout(Duration::from_secs(2)).unwrap();
        thread.join().unwrap();
    }
    #[test]
    fn rollover_failure_is_returned_before_appending() {
        let root =
            std::env::temp_dir().join(format!("essim-rollover-failure-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let mut sink = Sink::open(
            &root,
            "test",
            Limits {
                file: 10,
                history: 100,
            },
        )
        .unwrap();
        sink.write(b"123456789\n").unwrap();
        fs::write(sink.dir.join("application.jsonl.8"), b"archive").unwrap();
        fs::create_dir(sink.dir.join("application.jsonl.9")).unwrap();
        fs::write(sink.dir.join("application.jsonl.9/block"), b"blocked").unwrap();
        assert!(sink.write(b"next\n").is_err());
        assert!(!fs::read(sink.dir.join("application.jsonl"))
            .unwrap_or_default()
            .windows(4)
            .any(|w| w == b"next"));
    }
}
