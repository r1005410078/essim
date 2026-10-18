//! 资源策略通过受控执行器和真实RunCoordinator用例验证，未调用OS或协议库。
use essim_core::{application::runtime::*, domain::foundation::*};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
#[derive(Default)]
struct Resources {
    held: BTreeMap<String, (RunId, EndpointId)>,
    configs: BTreeMap<EndpointId, Option<String>>,
    partial_fail: Option<EndpointId>,
    external: bool,
    stops: Vec<RunId>,
}
struct Executor(Rc<RefCell<Resources>>);
impl ProtocolExecutor for Executor {
    fn open(&mut self, s: &RunSnapshot) -> Result<(), Error> {
        let mut r = self.0.borrow_mut();
        if let Some(key) = r.configs.get(&s.endpoint).cloned().flatten() {
            if r.external {
                return Err(Error::new(
                    "RESOURCE_BUSY",
                    "系统报告资源不可用；外部占用者未知",
                ));
            }
            if let Some((_, owner)) = r.held.get(&key) {
                return Err(Error::new(
                    "RESOURCE_BUSY",
                    format!("资源实际持有者：{}", owner.0),
                ));
            }
            r.held.insert(key, (s.run.clone(), s.endpoint.clone()));
        }
        if r.partial_fail.as_ref() == Some(&s.endpoint) {
            return Err(Error::new("OPEN_FAILED", "部分获取后失败"));
        }
        Ok(())
    }
    fn execute(&mut self, _: &RunId, _: &str) -> Result<ExecutionResult, Error> {
        Ok(ExecutionResult {
            confirmed: true,
            detail: "controlled".into(),
        })
    }
    fn request_stop(&mut self, id: &RunId) -> Result<(), Error> {
        let mut r = self.0.borrow_mut();
        r.stops.push(id.clone());
        r.held.retain(|_, (run, _)| run != id);
        Ok(())
    }
}
struct Recorder;
impl RecorderPort for Recorder {
    fn create(&mut self, _: &RunSnapshot) -> Result<(), Error> {
        Ok(())
    }
    fn append(&mut self, _: &HistoryEvent) -> Result<(), Error> {
        Ok(())
    }
}
struct TestClock;
impl Clock for TestClock {
    fn monotonic_ms(&self) -> u64 {
        10
    }
    fn unix_ms(&self) -> u64 {
        10
    }
}
fn snapshot(run: &str, endpoint: &str) -> RunSnapshot {
    RunSnapshot {
        run: RunId(run.into()),
        endpoint: EndpointId(endpoint.into()),
        units: vec![
            UnitId(format!("{endpoint}-u1")),
            UnitId(format!("{endpoint}-u2")),
        ],
        models: vec![],
        initial_values: BTreeMap::new(),
        serial_path: None,
    }
}
#[test]
fn tcp_listener_reports_actual_holder_but_masters_share_remote() {
    let r = Rc::new(RefCell::new(Resources::default()));
    for id in ["a", "b"] {
        r.borrow_mut()
            .configs
            .insert(EndpointId(id.into()), Some("tcp:127.0.0.1:1502".into()));
    }
    let mut runs = RunCoordinator::new(Executor(r.clone()), Recorder, TestClock);
    runs.start(snapshot("run-a", "a")).unwrap();
    let err = runs.start(snapshot("run-b", "b")).unwrap_err();
    assert!(err.message.contains("a"));
    assert_eq!(r.borrow().held.len(), 1);
    assert_eq!(
        runs.get(&RunId("run-a".into())).unwrap().lifecycle,
        RunLifecycle::Running
    );
    // 两个主站没有监听资源键；同一个远端不作为监听冲突。
    for id in ["master-a", "master-b"] {
        r.borrow_mut().configs.insert(EndpointId(id.into()), None);
        runs.start(snapshot(id, id)).unwrap();
    }
    assert_eq!(r.borrow().held.len(), 1);
}
#[test]
fn partial_open_failure_cleans_only_its_handles_and_external_owner_is_not_invented() {
    let r = Rc::new(RefCell::new(Resources::default()));
    r.borrow_mut()
        .configs
        .insert(EndpointId("ok".into()), Some("serial:A".into()));
    r.borrow_mut()
        .configs
        .insert(EndpointId("fail".into()), Some("serial:B".into()));
    r.borrow_mut().partial_fail = Some(EndpointId("fail".into()));
    let mut runs = RunCoordinator::new(Executor(r.clone()), Recorder, TestClock);
    runs.start(snapshot("ok", "ok")).unwrap();
    assert!(runs.start(snapshot("fail", "fail")).is_err());
    assert_eq!(r.borrow().held.len(), 1);
    assert!(r.borrow().held.contains_key("serial:A"));
    assert!(r.borrow().stops.contains(&RunId("fail".into())));
    r.borrow_mut().configs.insert(
        EndpointId("external-endpoint".into()),
        Some("tcp:external".into()),
    );
    r.borrow_mut().external = true;
    let err = runs
        .start(snapshot("external", "external-endpoint"))
        .unwrap_err();
    assert_eq!(err.code, "RESOURCE_BUSY");
    assert!(err.message.contains("外部占用者未知"));
}
#[test]
fn one_rtu_channel_shares_units_but_another_endpoint_is_exclusive() {
    let r = Rc::new(RefCell::new(Resources::default()));
    let mut runs = RunCoordinator::new(Executor(r.clone()), Recorder, TestClock);
    let mut a = snapshot("a", "a");
    a.serial_path = Some("/dev/controlled".into());
    let mut b = snapshot("b", "b");
    b.serial_path = a.serial_path.clone();
    runs.start(a).unwrap();
    assert_eq!(
        runs.get(&RunId("a".into())).unwrap().snapshot.units.len(),
        2
    );
    assert_eq!(runs.start(b).unwrap_err().code, "RESOURCE_BUSY");
}
