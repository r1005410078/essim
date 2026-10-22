//! 公共端点门拦截手动操作，任务查询和其他端点保持可用。
use essim_core::{
    application::{
        commands::{
            Action, Command,
            batch_targets::{BatchPlan, BatchScope, prepare_batch},
        },
        ports::{ApplyBoundary, ApplyState},
    },
    domain::{
        aggregates::batch::{BatchOperation, Outcome},
        endpoint::EndpointConfig,
        error::Error,
        identity::EndpointId,
    },
};
use essim_infrastructure::{
    batch_runner::{BatchExecutor, BatchRunner, WaitLimits},
    operation_gates::OperationPermit,
    workspace::WorkspaceEngine,
};
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
#[derive(Default)]
struct Control {
    entered: AtomicBool,
    released: Mutex<bool>,
    wake: Condvar,
}
struct Boundary(Arc<Control>);
impl ApplyBoundary for Boundary {
    fn stop(&mut self, _: &EndpointConfig) -> Result<(), Error> {
        self.0.entered.store(true, Ordering::SeqCst);
        let mut released = self.0.released.lock().unwrap();
        while !*released {
            released = self.0.wake.wait(released).unwrap();
        }
        Ok(())
    }
    fn state(&self, _: &EndpointId) -> ApplyState {
        if *self.0.released.lock().unwrap() {
            ApplyState::Stopped
        } else {
            ApplyState::Releasing
        }
    }
}
struct Adapter(Arc<WorkspaceEngine>);
impl BatchExecutor for Adapter {
    fn execute(&self, plan: &BatchPlan, id: &EndpointId, permit: &OperationPermit) -> Outcome {
        let result = self.0.revalidate_batch(plan, id, permit).and_then(|_| {
            self.0.execute_with_event_permitted(
                Command {
                    command_id: "batch-apply".into(),
                    action: Action::Apply {
                        id: id.clone(),
                        reset_incompatible: false,
                    },
                },
                permit,
            )
        });
        match result {
            Ok(_) => Outcome::Succeeded,
            Err(e) => Outcome::Failed(e.message),
        }
    }
}
fn setup(e: &WorkspaceEngine, id: &str) {
    e.execute(Command {
        command_id: format!("model-{id}"),
        action: Action::SaveModel {
            model: serde_json::from_value(
                serde_json::json!({"id":format!("m-{id}"),"name":"M","points":[]}),
            )
            .unwrap(),
        },
    })
    .unwrap();
    e.execute(Command{command_id:format!("create-{id}"),action:Action::CreateEndpoint{endpoint:serde_json::from_value(serde_json::json!({"id":id,"name":id,"role":"slave","transport":{"kind":"tcp","address":"127.0.0.1","port":1080},"units":[{"id":format!("u-{id}"),"name":"U","station":1,"enabled":true,"model":{"id":format!("m-{id}")},"device":null}]})).unwrap()}}).unwrap();
}
fn wait(f: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !f() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
}
struct Release(Arc<Control>);
impl Drop for Release {
    fn drop(&mut self) {
        *self.0.released.lock().unwrap() = true;
        self.0.wake.notify_all();
    }
}
#[test]
fn real_apply_wait_keeps_gate_while_queries_and_other_endpoint_changes_continue() {
    let control = Arc::new(Control::default());
    let engine = Arc::new(WorkspaceEngine::with_boundary(Box::new(Boundary(
        control.clone(),
    ))));
    setup(&engine, "a");
    let plan = prepare_batch(
        &engine.query().unwrap().catalog,
        "batch".into(),
        BatchOperation::Apply,
        BatchScope::Selected {
            ids: vec![EndpointId("a".into())],
        },
        1,
    )
    .unwrap();
    let runner = BatchRunner::with_operations(
        Arc::new(Adapter(engine.clone())),
        WaitLimits {
            apply: Duration::from_millis(5),
            start: Duration::from_secs(1),
            stop: Duration::from_secs(1),
        },
        1024,
        8 * 1024 * 1024,
        engine.operation_gates(),
    )
    .unwrap();
    let _release = Release(control.clone());
    runner.submit(plan).unwrap();
    wait(|| control.entered.load(Ordering::SeqCst));
    wait(|| runner.query().unwrap().unwrap().counts.uncertain == 1);
    assert_eq!(
        engine
            .execute(Command {
                command_id: "duplicate".into(),
                action: Action::Apply {
                    id: EndpointId("a".into()),
                    reset_incompatible: false
                }
            })
            .unwrap_err()
            .code,
        "ENDPOINT_BUSY"
    );
    assert!(engine.query().unwrap().catalog.applied.is_empty());
    setup(&engine, "b");
    assert_eq!(runner.query().unwrap().unwrap().occupied, 1);
    runner.cancel("batch").unwrap();
    assert_eq!(runner.query().unwrap().unwrap().occupied, 1);
    *control.released.lock().unwrap() = true;
    control.wake.notify_all();
    wait(|| runner.query().unwrap().unwrap().finished);
    assert!(
        engine
            .operation_gates()
            .ensure_available(&EndpointId("a".into()))
            .is_ok()
    );
    // 无关目录保存应合并保留，不应使健康目标的应用失败。
    let view = runner.query().unwrap().unwrap();
    assert_eq!(view.counts.succeeded, 1);
    assert_eq!(view.counts.failed, 0);
    assert_eq!(engine.query().unwrap().catalog.applied.len(), 1);
    assert_eq!(engine.query().unwrap().catalog.endpoints.len(), 2);
}
