//! 受限候选StateScope：两阶段可靠记录通过实际RecordingPool；不装配生产路径。
use essim_infrastructure::{
    recording::{DurableSink, RecordCompletion, RecordStream, RecordingPool},
    resources::ResourceManager,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{self, Write},
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
static NEXT: AtomicU64 = AtomicU64::new(1);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Active,
    Quiescing,
    RecoveryRequired,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    Pending,
    Decided,
    Committed,
    Rejected,
    Unknown,
}
#[derive(Clone, Copy, Debug)]
enum Source {
    Local,
    Remote,
    Sample,
    DeviceCommand,
}
#[derive(Clone, Copy, Debug)]
struct Handle {
    scope: u64,
    epoch: u64,
}
#[derive(Clone, Copy, Debug)]
struct Fence {
    handle: Handle,
    quiesce: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    epoch: u64,
    cursor: u64,
    values: BTreeMap<String, i64>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Intent,
    Decision,
    Publish,
    Unknown,
}
struct Pending {
    id: u64,
    values: BTreeMap<String, i64>,
    phase: Phase,
    completion: Option<RecordCompletion>,
}
struct Scope {
    id: u64,
    epoch: u64,
    cursor: u64,
    next: u64,
    mode: Mode,
    next_quiesce: u64,
    active_quiesce: Option<u64>,
    values: BTreeMap<String, i64>,
    stream: RecordStream,
    sequence: u64,
    pending: Option<Pending>,
    status: BTreeMap<u64, Status>,
    bindings: Vec<&'static str>,
    projections: Vec<(u64, &'static str, Snapshot)>,
    receipts: BTreeSet<(u64, &'static str)>,
    actions: BTreeSet<u64>,
    lose_decision_ack: bool,
}
impl Scope {
    fn new(stream: RecordStream, bindings: Vec<&'static str>) -> Self {
        stream
            .append(
                b"{\"kind\":\"scope_snapshot\",\"schema\":[\"p\"],\"initial\":0}\n".to_vec(),
                1,
            )
            .unwrap()
            .wait_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap()
            .unwrap();
        Self {
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            epoch: 1,
            cursor: 0,
            next: 0,
            mode: Mode::Active,
            next_quiesce: 0,
            active_quiesce: None,
            values: BTreeMap::from([("p".into(), 0)]),
            stream,
            sequence: 1,
            pending: None,
            status: BTreeMap::new(),
            bindings,
            projections: vec![],
            receipts: BTreeSet::new(),
            actions: BTreeSet::new(),
            lose_decision_ack: false,
        }
    }
    fn handle(&self) -> Handle {
        Handle {
            scope: self.id,
            epoch: self.epoch,
        }
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            epoch: self.epoch,
            cursor: self.cursor,
            values: self.values.clone(),
        }
    }
    fn validate_handle(&self, h: Handle) -> Result<(), &'static str> {
        if h.scope != self.id || h.epoch != self.epoch {
            Err("StaleScope")
        } else {
            Ok(())
        }
    }
    fn submit_patch(
        &mut self,
        h: Handle,
        _source: Source,
        patch: Vec<(&str, i64)>,
    ) -> Result<u64, &'static str> {
        self.validate_handle(h)?;
        if self.mode != Mode::Active {
            return Err("ScopeQuiescing");
        }
        self.prepare(patch)
    }
    fn prepare(&mut self, patch: Vec<(&str, i64)>) -> Result<u64, &'static str> {
        if self.pending.is_some() {
            return Err("CommitBusy");
        }
        let mut seen = BTreeSet::new();
        let mut values = self.values.clone();
        for (key, value) in patch {
            if !values.contains_key(key) || !seen.insert(key) {
                return Err("InvalidPatch");
            }
            values.insert(key.into(), value);
        }
        self.next += 1;
        let id = self.next;
        // RecordStream受理失败不推进其连续前缀，本地序号也必须等受理成功再推进。
        let sequence = self.sequence + 1;
        let completion = self
            .stream
            .append(
                format!("{{\"kind\":\"intent\",\"id\":{id}}}\n").into_bytes(),
                sequence,
            )
            .map_err(|_| "JournalRejected")?;
        self.sequence = sequence;
        self.status.insert(id, Status::Pending);
        self.pending = Some(Pending {
            id,
            values,
            phase: Phase::Intent,
            completion: Some(completion),
        });
        Ok(id)
    }
    fn poll(&mut self) {
        let Some(mut p) = self.pending.take() else {
            return;
        };
        if matches!(p.phase, Phase::Publish | Phase::Unknown) {
            self.pending = Some(p);
            return;
        }
        let result = p.completion.as_ref().unwrap().wait_timeout(Duration::ZERO);
        match result {
            Ok(None) => {}
            Ok(Some(Ok(_))) if p.phase == Phase::Intent => {
                let sequence = self.sequence + 1;
                let bytes=serde_json::to_vec(&serde_json::json!({"kind":"decided","id":p.id,"cursor":self.cursor+1,"values":p.values})).unwrap();
                match self
                    .stream
                    .append([bytes, b"\n".to_vec()].concat(), sequence)
                {
                    Ok(c) => {
                        self.sequence = sequence;
                        p.phase = Phase::Decision;
                        p.completion = Some(c);
                    }
                    Err(_) => {
                        self.status.insert(p.id, Status::Rejected);
                        self.mode = Mode::RecoveryRequired;
                        return;
                    }
                }
            }
            Ok(Some(Ok(_))) if !self.lose_decision_ack => {
                p.phase = Phase::Publish;
                p.completion = None;
                self.status.insert(p.id, Status::Decided);
            }
            _ => {
                let unknown = p.phase == Phase::Decision;
                self.status.insert(
                    p.id,
                    if unknown {
                        Status::Unknown
                    } else {
                        Status::Rejected
                    },
                );
                self.mode = Mode::RecoveryRequired;
                if !unknown {
                    return;
                }
                p.phase = Phase::Unknown;
                p.completion = None;
            }
        }
        self.pending = Some(p);
    }
    fn publish(&mut self) -> Result<(), &'static str> {
        if self.pending.as_ref().map(|p| p.phase) != Some(Phase::Publish) {
            return Err("NotDecided");
        }
        let p = self.pending.take().unwrap();
        self.values = p.values;
        self.cursor += 1;
        self.status.insert(p.id, Status::Committed);
        for binding in &self.bindings {
            self.projections.push((p.id, binding, self.snapshot()));
            self.receipts.insert((p.id, binding));
        }
        Ok(())
    }
    fn begin_quiesce(&mut self) -> Result<Fence, &'static str> {
        if self.mode != Mode::Active {
            return Err("RecoveryRequired");
        }
        let quiesce = self.next_quiesce.checked_add(1).ok_or("QuiesceExhausted")?;
        self.next_quiesce = quiesce;
        self.active_quiesce = Some(quiesce);
        self.mode = Mode::Quiescing;
        Ok(Fence {
            handle: self.handle(),
            quiesce,
        })
    }
    fn final_snapshot(&self, f: Fence) -> Result<Snapshot, &'static str> {
        self.validate_handle(f.handle)?;
        if self.active_quiesce != Some(f.quiesce) {
            return Err("StaleFence");
        }
        if self.mode != Mode::Quiescing {
            return Err("RecoveryRequired");
        }
        if self.pending.is_some() || !self.actions.is_empty() || !self.receipts.is_empty() {
            return Err("Draining");
        }
        Ok(self.snapshot())
    }
    fn publish_migration(&mut self, f: Fence, final_state: &Snapshot) -> Result<(), &'static str> {
        if &self.final_snapshot(f)? != final_state {
            return Err("FinalSnapshotMismatch");
        }
        self.epoch += 1;
        self.active_quiesce = None;
        self.cursor = 0;
        self.mode = Mode::Active;
        Ok(())
    }
    fn abort_quiesce(&mut self, f: Fence) -> Result<(), &'static str> {
        self.final_snapshot(f)?;
        self.active_quiesce = None;
        self.mode = Mode::Active;
        Ok(())
    }
    fn admit_action(&mut self, h: Handle) -> Result<u64, &'static str> {
        self.validate_handle(h)?;
        if self.mode != Mode::Active {
            return Err("ScopeQuiescing");
        }
        self.next += 1;
        self.actions.insert(self.next);
        Ok(self.next)
    }
    fn finish_action(&mut self, id: u64, value: i64) -> Result<u64, &'static str> {
        if !self.actions.contains(&id) {
            return Err("UnknownAction");
        }
        if self.mode == Mode::RecoveryRequired {
            return Err("RecoveryRequired");
        }
        let commit = self.prepare(vec![("p", value)])?;
        self.actions.remove(&id);
        Ok(commit)
    }
}
struct SyncControl {
    stage: AtomicUsize,
    pause: usize,
    fail: usize,
    released: Mutex<bool>,
    wake: Condvar,
}
struct Sink {
    file: File,
    control: Arc<SyncControl>,
}
impl Write for Sink {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.file.write(b)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}
impl DurableSink for Sink {
    fn sync_data(&mut self) -> io::Result<()> {
        let stage = self.control.stage.fetch_add(1, Ordering::SeqCst) + 1;
        if stage == self.control.pause {
            let mut r = self.control.released.lock().unwrap();
            while !*r {
                r = self.control.wake.wait(r).unwrap();
            }
        }
        self.file.sync_data()?;
        if stage == self.control.fail {
            Err(io::Error::other("故意在真实sync后丢失成功确认"))
        } else {
            Ok(())
        }
    }
}
struct Env {
    path: PathBuf,
    control: Arc<SyncControl>,
    pool: RecordingPool,
}
impl Env {
    fn new(pause: usize, fail: usize) -> Self {
        Self {
            path: std::env::temp_dir().join(format!(
                "essim-g0-{}-{}.jsonl",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )),
            control: Arc::new(SyncControl {
                stage: AtomicUsize::new(0),
                pause,
                fail,
                released: Mutex::new(false),
                wake: Condvar::new(),
            }),
            pool: RecordingPool::new(ResourceManager::default()).unwrap(),
        }
    }
    fn scope(&self, bindings: Vec<&'static str>) -> Scope {
        Scope::new(
            self.pool
                .stream(Box::new(Sink {
                    file: File::create(&self.path).unwrap(),
                    control: self.control.clone(),
                }))
                .unwrap(),
            bindings,
        )
    }
    fn release(&self) {
        *self.control.released.lock().unwrap() = true;
        self.control.wake.notify_all();
    }
    fn records(&self) -> Vec<serde_json::Value> {
        fs::read_to_string(&self.path)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
}
impl Drop for Env {
    fn drop(&mut self) {
        self.release();
        let _ = fs::remove_file(&self.path);
    }
}
fn wait(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !predicate() {
        assert!(Instant::now() < deadline, "G0等待超时");
        std::thread::yield_now();
    }
}
fn decided(scope: &mut Scope, id: u64) {
    wait(|| {
        scope.poll();
        scope.status[&id] != Status::Pending
    });
}

#[test]
fn record_queue_rejection_preserves_sequence_and_recovers_after_budget_release() {
    let resources = ResourceManager::default();
    let mut env = Env::new(0, 0);
    env.pool = RecordingPool::new(resources.clone()).unwrap();
    let mut scope = env.scope(vec!["104"]);
    wait(|| resources.query().unwrap().current.queue_bytes == 0);
    // 只占账本额度，不分配64MiB；使真实RecordingPool在受理前明确拒绝。
    let hold = resources
        .reserve(essim_core::domain::resources::ResourceUsage {
            queue_bytes: resources.policy().limit.queue_bytes,
            ..Default::default()
        })
        .unwrap();
    for _ in 0..2 {
        assert_eq!(
            scope.submit_patch(scope.handle(), Source::Local, vec![("p", 5)]),
            Err("JournalRejected")
        );
        assert!(scope.pending.is_none());
        assert!(scope.status.is_empty());
        assert!(scope.projections.is_empty());
        assert_eq!(scope.snapshot().cursor, 0);
        assert_eq!(scope.snapshot().values["p"], 0);
        assert_eq!(scope.sequence, 1);
        assert_eq!(scope.stream.reliable().unwrap(), 1);
        assert_eq!(env.records().len(), 1);
    }
    drop(hold);
    let resumed = scope.submit_patch(scope.handle(), Source::Local, vec![("p", 6)]);
    println!(
        "record-admission recovery: result={resumed:?}, sequence={}, reliable={}",
        scope.sequence,
        scope.stream.reliable().unwrap()
    );
    let id = resumed.expect("未受理的记录不能消耗序号，额度释放后必须可重新提交");
    assert_eq!(scope.sequence, 2);
    assert_eq!(scope.mode, Mode::Active);
    assert_eq!(scope.snapshot().values["p"], 0);
    assert!(scope.projections.is_empty());
    decided(&mut scope, id);
    assert_eq!(scope.stream.reliable().unwrap(), 3);
    assert_eq!(scope.snapshot().cursor, 0);
    scope.publish().unwrap();
    assert_eq!(scope.snapshot().values["p"], 6);
    assert_eq!(scope.snapshot().cursor, 1);
    assert_eq!(scope.projections.len(), 1);
    let records = env.records();
    assert_eq!(records.len(), 3);
    assert_eq!(records[1]["kind"], "intent");
    assert_eq!(records[1]["id"], id);
    assert_eq!(records[2]["kind"], "decided");
    assert_eq!(records[2]["id"], id);
    // 再完成一个独立提交，确认恢复后连续前缀没有残留断档。
    let next = scope
        .submit_patch(scope.handle(), Source::Remote, vec![("p", 7)])
        .unwrap();
    decided(&mut scope, next);
    scope.publish().unwrap();
    assert_eq!(scope.sequence, 5);
    assert_eq!(scope.stream.reliable().unwrap(), 5);
    assert_eq!(scope.snapshot().cursor, 2);
    assert_eq!(scope.snapshot().values["p"], 7);
    assert_eq!(scope.projections.len(), 2);
    println!("record-admission recovered: reliable=5, cursor=2, value=7, projections=2");
}

#[test]
fn durable_decision_precedes_atomic_publication_and_each_projection_receipt() {
    let env = Env::new(3, 0);
    let mut scope = env.scope(vec!["modbus", "104"]);
    let h = scope.handle();
    assert!(
        scope
            .submit_patch(h, Source::Local, vec![("p", 7), ("missing", 9)])
            .is_err()
    );
    assert!(
        scope
            .submit_patch(h, Source::Local, vec![("p", 7), ("p", 9)])
            .is_err()
    );
    assert_eq!(env.records().len(), 1);
    let id = scope
        .submit_patch(h, Source::Remote, vec![("p", 7)])
        .unwrap();
    wait(|| {
        scope.poll();
        env.control.stage.load(Ordering::SeqCst) == 3
    });
    assert_eq!(scope.snapshot().values["p"], 0);
    assert!(scope.projections.is_empty());
    assert!(scope.publish().is_err());
    env.release();
    decided(&mut scope, id);
    assert_eq!(scope.status[&id], Status::Decided);
    assert_eq!(scope.snapshot().cursor, 0);
    scope.publish().unwrap();
    assert_eq!(scope.snapshot().values["p"], 7);
    assert_eq!(scope.snapshot().cursor, 1);
    let records = env.records();
    assert_eq!(records.iter().filter(|r| r["kind"] == "decided").count(), 1);
    assert_eq!(scope.projections.len(), 2);
    assert!(
        scope
            .projections
            .iter()
            .all(|(commit, _, snapshot)| *commit == id && snapshot == &scope.snapshot())
    );
    assert_eq!(scope.receipts.len(), 2); // StateCommitted仍不等于源协议回复完成。
    scope.receipts.remove(&(id, "104")); // 确定投影失败，不回滚已提交值。
    assert_eq!(scope.snapshot().values["p"], 7);
    assert_eq!(scope.receipts.len(), 1);
}
#[test]
fn intent_failure_never_decides_and_unknown_decision_blocks_writes_and_migration() {
    for fail in [2, 3] {
        let env = Env::new(0, fail);
        let mut scope = env.scope(vec!["104"]);
        let h = scope.handle();
        let id = scope
            .submit_patch(h, Source::Local, vec![("p", 5)])
            .unwrap();
        decided(&mut scope, id);
        assert_eq!(
            scope.status[&id],
            if fail == 2 {
                Status::Rejected
            } else {
                Status::Unknown
            }
        );
        assert_eq!(scope.snapshot().values["p"], 0);
        assert!(scope.projections.is_empty());
        assert!(scope.publish().is_err());
        assert!(
            scope
                .submit_patch(h, Source::Remote, vec![("p", 9)])
                .is_err()
        );
        assert!(scope.begin_quiesce().is_err());
        if fail == 3 {
            assert_eq!(
                env.records()
                    .iter()
                    .filter(|r| r["kind"] == "decided")
                    .count(),
                1,
                "sync确认失败不能推断决策未落盘"
            );
        }
    }
}
#[test]
fn lost_success_ack_keeps_unknown_without_duplicate_decision_or_projection() {
    let env = Env::new(0, 0);
    let mut scope = env.scope(vec!["104"]);
    scope.lose_decision_ack = true;
    let id = scope
        .submit_patch(scope.handle(), Source::Sample, vec![("p", 8)])
        .unwrap();
    decided(&mut scope, id);
    for _ in 0..10 {
        scope.poll();
    }
    assert_eq!(scope.status[&id], Status::Unknown);
    assert_eq!(scope.snapshot().cursor, 0);
    assert!(scope.projections.is_empty());
    assert_eq!(
        env.records()
            .iter()
            .filter(|r| r["kind"] == "decided")
            .count(),
        1
    );
}
#[test]
fn crash_cut_after_decision_retains_evidence_without_replaying_network() {
    let env = Env::new(0, 0);
    let mut scope = env.scope(vec!["modbus", "104"]);
    let id = scope
        .submit_patch(scope.handle(), Source::Local, vec![("p", 11)])
        .unwrap();
    decided(&mut scope, id);
    assert!(scope.projections.is_empty());
    drop(scope); // 受控模拟发布前所有者丢失，不声称实际kill进程。
    let facts = env.records();
    let decided = facts.iter().find(|r| r["kind"] == "decided").unwrap();
    assert_eq!(decided["id"], id);
    assert_eq!(decided["values"]["p"], 11);
    assert!(facts.iter().all(|r| r["kind"] != "sent"));
}
#[test]
fn scope_fence_covers_all_sources_drains_actions_and_uses_final_snapshot() {
    let env = Env::new(0, 0);
    let mut scope = env.scope(vec!["104"]);
    let h = scope.handle();
    let preview = scope.snapshot();
    let action = scope.admit_action(h).unwrap();
    let fence = scope.begin_quiesce().unwrap();
    for source in [
        Source::Local,
        Source::Remote,
        Source::Sample,
        Source::DeviceCommand,
    ] {
        assert_eq!(
            scope.submit_patch(h, source, vec![("p", 99)]),
            Err("ScopeQuiescing")
        );
    }
    assert_eq!(scope.admit_action(h), Err("ScopeQuiescing"));
    assert_eq!(scope.final_snapshot(fence), Err("Draining"));
    let id = scope.finish_action(action, 12).unwrap();
    assert!(scope.final_snapshot(fence).is_err());
    decided(&mut scope, id);
    scope.publish().unwrap();
    assert!(
        scope.final_snapshot(fence).is_err(),
        "协议资源释放不能替代投影排空"
    );
    scope.receipts.clear();
    let final_state = scope.final_snapshot(fence).unwrap();
    assert_eq!(final_state.values["p"], 12);
    assert_ne!(preview, final_state);
    assert_eq!(
        scope.publish_migration(fence, &preview),
        Err("FinalSnapshotMismatch")
    );
    scope.publish_migration(fence, &final_state).unwrap();
    assert_eq!(scope.snapshot().values["p"], 12);
    assert_eq!(
        scope.submit_patch(h, Source::Remote, vec![("p", 13)]),
        Err("StaleScope")
    );
}
#[test]
fn admitted_commit_drains_after_fence_and_unknown_cannot_be_force_aborted() {
    let env = Env::new(0, 3);
    let mut scope = env.scope(vec![]);
    let h = scope.handle();
    let id = scope
        .submit_patch(h, Source::Local, vec![("p", 6)])
        .unwrap();
    let fence = scope.begin_quiesce().unwrap();
    decided(&mut scope, id);
    assert_eq!(scope.status[&id], Status::Unknown);
    assert!(scope.final_snapshot(fence).is_err());
    assert!(scope.abort_quiesce(fence).is_err());
    assert!(
        scope
            .submit_patch(h, Source::DeviceCommand, vec![("p", 7)])
            .is_err()
    );
}
#[test]
fn scope_without_protocol_run_can_commit_and_definite_apply_abort_keeps_values() {
    let env = Env::new(0, 0);
    let mut scope = env.scope(vec![]);
    let id = scope
        .submit_patch(scope.handle(), Source::Local, vec![("p", 4)])
        .unwrap();
    decided(&mut scope, id);
    scope.publish().unwrap();
    let fence = scope.begin_quiesce().unwrap();
    scope.abort_quiesce(fence).unwrap();
    assert_eq!(scope.snapshot().values["p"], 4);
    assert!(env.records()[0].get("runId").is_none());
}

#[test]
fn old_fence_cannot_abort_inspect_or_publish_a_new_quiesce() {
    let env = Env::new(0, 0);
    let mut scope = env.scope(vec![]);
    let first = scope.begin_quiesce().unwrap();
    scope.abort_quiesce(first).unwrap();
    let second = scope.begin_quiesce().unwrap();
    let snapshot = scope.final_snapshot(second).unwrap();
    assert!(
        scope.final_snapshot(first).is_err(),
        "旧fence不能读取新一轮最终快照"
    );
    assert!(
        scope.publish_migration(first, &snapshot).is_err(),
        "旧fence不能发布新一轮迁移"
    );
    assert!(
        scope.abort_quiesce(first).is_err(),
        "旧fence不能解除新一轮栅栏"
    );
    assert_eq!(
        scope.submit_patch(scope.handle(), Source::Remote, vec![("p", 123)]),
        Err("ScopeQuiescing")
    );
    scope.abort_quiesce(second).unwrap();
    assert!(scope.abort_quiesce(second).is_err());
    let third = scope.begin_quiesce().unwrap();
    let snapshot = scope.final_snapshot(third).unwrap();
    scope.publish_migration(third, &snapshot).unwrap();
    assert!(scope.final_snapshot(third).is_err());
    assert!(scope.abort_quiesce(third).is_err());
}
