//! 可控端口只存在于测试；这些断言不是可靠落盘或实际协议验收。
use essim_core::{application::runtime::*, domain::foundation::*};
use std::{cell::RefCell, rc::Rc};
#[derive(Default)]
struct Fake {
    calls: Vec<String>,
    fail: Option<String>,
}
#[derive(Clone)]
struct Executor(Rc<RefCell<Fake>>);
impl ProtocolExecutor for Executor {
    fn open(&mut self, _: &RunSnapshot) -> Result<(), Error> {
        self.0.borrow_mut().calls.push("open".into());
        Ok(())
    }
    fn execute(&mut self, _: &RunId, _: &str) -> Result<ExecutionResult, Error> {
        self.0.borrow_mut().calls.push("write".into());
        Ok(ExecutionResult {
            confirmed: true,
            detail: "test".into(),
        })
    }
    fn request_stop(&mut self, _: &RunId) -> Result<(), Error> {
        self.0.borrow_mut().calls.push("stop".into());
        Ok(())
    }
}
struct Recorder(Rc<RefCell<Fake>>);
impl RecorderPort for Recorder {
    fn create(&mut self, _: &RunSnapshot) -> Result<(), Error> {
        self.step("create")
    }
    fn append(&mut self, event: &HistoryEvent) -> Result<(), Error> {
        self.step(match event.fact {
            Fact::Intent { .. } => "intent",
            Fact::Result { .. } => "result",
            _ => "fact",
        })
    }
}
impl Recorder {
    fn step(&self, step: &str) -> Result<(), Error> {
        let mut fake = self.0.borrow_mut();
        fake.calls.push(step.into());
        if fake.fail.as_deref() == Some(step) {
            Err(Error::new("RECORD_FAILED", step))
        } else {
            Ok(())
        }
    }
}
struct TestClock;
impl Clock for TestClock {
    fn monotonic_ms(&self) -> u64 {
        10
    }
    fn unix_ms(&self) -> u64 {
        100
    }
}
fn snapshot(id: &str) -> RunSnapshot {
    RunSnapshot {
        run: RunId(id.into()),
        endpoint: EndpointId("endpoint".into()),

        units: vec![UnitId("a".into()), UnitId("b".into())],
        models: vec![],
        initial_values: Default::default(),
        serial_path: None,
    }
}
#[test]
fn create_failure_never_opens_executor() {
    let fake = Rc::new(RefCell::new(Fake {
        fail: Some("create".into()),
        ..Default::default()
    }));
    let mut runs = RunCoordinator::new(Executor(fake.clone()), Recorder(fake.clone()), TestClock);
    assert!(runs.start(snapshot("run-a")).is_err());
    assert!(!fake.borrow().calls.iter().any(|c| c == "open"));
}
#[test]
fn intent_failure_prevents_write_and_result_failure_never_resends() {
    for failure in ["intent", "result"] {
        let fake = Rc::new(RefCell::new(Fake::default()));
        let mut runs =
            RunCoordinator::new(Executor(fake.clone()), Recorder(fake.clone()), TestClock);
        runs.start(snapshot("run-a")).unwrap();
        fake.borrow_mut().fail = Some(failure.into());
        assert!(runs.perform(&RunId("run-a".into()), "operation-a").is_err());
        let calls = &fake.borrow().calls;
        assert_eq!(
            calls.iter().filter(|c| c.as_str() == "write").count(),
            usize::from(failure == "result")
        );
        assert_eq!(calls.iter().filter(|c| c.as_str() == "stop").count(), 1);
        let run = runs.get(&RunId("run-a".into())).unwrap();
        assert_eq!(run.lifecycle, RunLifecycle::Stopping);
        assert_eq!(run.snapshot.units.len(), 2);
        assert_eq!(run.uncertain, failure == "result");
        assert!(run.fault.is_some());
    }
}
#[test]
fn replay_has_no_executor_and_rejects_gaps_or_schema_changes() {
    let mut replay = ReplaySession::new(ReplayId("replay-a".into()), snapshot("run-a"));
    let event = HistoryEvent {
        schema: 1,
        run: RunId("run-a".into()),
        sequence: 1,
        relative_ms: 0,
        unix_ms: 100,
        command_id: Some("op".into()),
        fact: Fact::Intent {
            operation: "op".into(),
        },
    };
    assert!(replay.accept(&event).unwrap());
    assert!(!replay.accept(&event).unwrap());
    assert!(replay.unconfirmed.contains("op"));
    let mut bad = event.clone();
    bad.sequence = 3;
    assert!(replay.accept(&bad).is_err());
    assert_eq!(replay.position, 1);
    bad.sequence = 2;
    bad.schema = 2;
    assert!(replay.accept(&bad).is_err());
}
#[test]
fn stop_requires_confirmation_reconnect_and_pause_keep_run_and_restart_needs_new_id() {
    let fake = Rc::new(RefCell::new(Fake::default()));
    let mut runs = RunCoordinator::new(Executor(fake.clone()), Recorder(fake.clone()), TestClock);
    let run = RunId("run-a".into());
    runs.start(snapshot("run-a")).unwrap();
    runs.connection(&run, false).unwrap();
    runs.connection(&run, true).unwrap();
    runs.pause_unit(&run, UnitId("a".into()), true).unwrap();
    assert_eq!(runs.get(&run).unwrap().snapshot.run, run);
    runs.stop(&run).unwrap();
    runs.stop(&run).unwrap();
    assert_eq!(runs.get(&run).unwrap().lifecycle, RunLifecycle::Stopping);
    assert_eq!(
        fake.borrow()
            .calls
            .iter()
            .filter(|c| c.as_str() == "stop")
            .count(),
        1
    );
    runs.confirm_stopped(&run).unwrap();
    runs.confirm_stopped(&run).unwrap();
    assert_eq!(runs.get(&run).unwrap().recording, RecordingState::Finalized);
    assert!(runs.start(snapshot("run-a")).is_err());
    runs.start(snapshot("run-b")).unwrap();
}
#[test]
fn shared_storage_fault_stops_every_affected_run_and_isolates_others() {
    let fake = Rc::new(RefCell::new(Fake::default()));
    let mut runs = RunCoordinator::new(Executor(fake.clone()), Recorder(fake.clone()), TestClock);
    for id in ["a", "b", "c"] {
        let mut config = snapshot(id);
        config.endpoint = EndpointId(id.into());
        runs.start(config).unwrap();
    }
    runs.storage_failed(
        &[RunId("a".into()), RunId("b".into())],
        Error::new("DISK_FAILED", "test"),
    );
    assert_eq!(
        runs.get(&RunId("a".into())).unwrap().lifecycle,
        RunLifecycle::Stopping
    );
    assert_eq!(
        runs.get(&RunId("b".into())).unwrap().lifecycle,
        RunLifecycle::Stopping
    );
    assert_eq!(
        runs.get(&RunId("c".into())).unwrap().lifecycle,
        RunLifecycle::Running
    );
    runs.check_stop_deadlines(0);
    assert_eq!(
        runs.get(&RunId("a".into())).unwrap().lifecycle,
        RunLifecycle::Failed
    );
    assert!(runs.perform(&RunId("a".into()), "late-op").is_err());
    runs.confirm_stopped(&RunId("a".into())).unwrap();
    assert_eq!(
        runs.get(&RunId("a".into())).unwrap().recording,
        RecordingState::Failed
    );
}
#[test]
fn replay_uses_fixed_snapshots_isolated_values_and_keeps_partial_intent() {
    use std::collections::BTreeMap;
    let unit = UnitId("a".into());
    let point = PointId("test".into());
    let mut config = snapshot("run-a");
    config.initial_values.insert(
        unit.clone(),
        UnitValues {
            model: Some(CollectionModel {
                id: ModelId("m".into()),
                name: "快照".into(),
                points: vec![],
            }),
            raw: BTreeMap::from([(point.clone(), RawValue::Unsigned(10))]),
            stale: false,
        },
    );
    let original = config.clone();
    let mut replay = ReplaySession::new(ReplayId("r".into()), config);
    let mut event = HistoryEvent {
        schema: 1,
        run: RunId("run-a".into()),
        sequence: 1,
        relative_ms: 0,
        unix_ms: 100,
        command_id: None,
        fact: Fact::Value {
            unit: unit.clone(),
            point: point.clone(),
            raw: RawValue::Unsigned(44),
        },
    };
    replay.accept(&event).unwrap();
    assert_eq!(
        original.initial_values[&unit].raw[&point],
        RawValue::Unsigned(10)
    );
    assert_eq!(replay.values[&unit].raw[&point], RawValue::Unsigned(44));
    assert_eq!(replay.values[&unit].model.as_ref().unwrap().name, "快照");
    event.sequence = 2;
    event.fact = Fact::Intent {
        operation: "unknown-result".into(),
    };
    replay.accept(&event).unwrap();
    assert!(!replay.finalized);
    assert!(replay.unconfirmed.contains("unknown-result"));
    let mut invalid = ReplaySession::new(ReplayId("other".into()), original);
    event.sequence = 1;
    event.schema = 7;
    assert_eq!(invalid.accept(&event).unwrap_err().code, "UNKNOWN_HISTORY");
    assert_eq!(invalid.position, 0);
}
#[test]
fn rtu_resource_claim_remains_exclusive_until_stop_confirmation() {
    let fake = Rc::new(RefCell::new(Fake::default()));
    let mut runs = RunCoordinator::new(Executor(fake.clone()), Recorder(fake.clone()), TestClock);
    let mut first = snapshot("first");
    first.serial_path = Some("test-serial".into());
    runs.start(first).unwrap();
    let mut second = snapshot("second");
    second.endpoint = EndpointId("other".into());
    second.serial_path = Some("test-serial".into());
    assert!(runs.start(second.clone()).is_err());
    runs.stop(&RunId("first".into())).unwrap();
    assert!(runs.start(second.clone()).is_err());
    runs.confirm_stopped(&RunId("first".into())).unwrap();
    runs.start(second).unwrap();
}
#[test]
fn failed_record_creation_does_not_reserve_a_channel_for_the_next_run() {
    let fake = Rc::new(RefCell::new(Fake {
        fail: Some("create".into()),
        ..Default::default()
    }));
    let mut runs = RunCoordinator::new(Executor(fake.clone()), Recorder(fake.clone()), TestClock);
    assert!(runs.start(snapshot("failed")).is_err());
    fake.borrow_mut().fail = None;
    assert!(runs.start(snapshot("next")).is_ok());
}
