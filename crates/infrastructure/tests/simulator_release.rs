//! 受控边界证据，不打开TCP或串口。等待时其他配置操作必须可继续。
use essim_core::{application::workspace::*, domain::foundation::*};
use essim_infrastructure::workspace::WorkspaceEngine;
use std::sync::{mpsc, Arc};
struct Boundary {
    entered: mpsc::Sender<()>,
    resume: mpsc::Receiver<bool>,
}
impl ApplyBoundary for Boundary {
    fn state(&self, _: &EndpointId) -> ApplyState {
        ApplyState::Failed
    }
    fn stop(&mut self, _: &EndpointConfig) -> Result<(), Error> {
        self.entered.send(()).unwrap();
        if self.resume.recv().unwrap() {
            Ok(())
        } else {
            Err(Error::new("RELEASE_TIMEOUT", "资源尚未确认释放"))
        }
    }
}
fn command(action: serde_json::Value) -> Command {
    serde_json::from_value(serde_json::json!({"commandId":"test","action":action})).unwrap()
}
fn seed(e: &WorkspaceEngine) {
    e.execute(command(
        serde_json::json!({"kind":"saveModel","model":{"id":"m","name":"M","points":[]}}),
    ))
    .unwrap();
    e.execute(command(serde_json::json!({"kind":"createEndpoint","endpoint":{"id":"e","name":"E","role":"slave","transport":{"kind":"tcp","address":"127.0.0.1","port":1502},"units":[{"id":"u","name":"U","station":1,"enabled":true,"device":null,"model":{"id":"m"}}]}}))).unwrap();
}
#[test]
fn waiting_release_does_not_lock_catalog_and_merges_unrelated_save() {
    let (entered, waiting) = mpsc::channel();
    let (resume, release) = mpsc::channel();
    let e = Arc::new(WorkspaceEngine::with_boundary(Box::new(Boundary {
        entered,
        resume: release,
    })));
    seed(&e);
    let worker = e.clone();
    let handle = std::thread::spawn(move || {
        worker.execute(command(
            serde_json::json!({"kind":"apply","id":"e","resetIncompatible":false}),
        ))
    });
    waiting
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    assert_eq!(
        e.query().unwrap().application_states[&EndpointId("e".into())],
        ApplyState::Releasing
    );
    e.execute(command(
        serde_json::json!({"kind":"createDevice","device":{"id":"d","name":"D","kind":"PCS"}}),
    ))
    .unwrap();
    resume.send(true).unwrap();
    handle.join().unwrap().unwrap();
    let view = e.query().unwrap();
    assert_eq!(view.catalog.devices.len(), 1);
    assert_eq!(view.catalog.applied.len(), 1);
}
#[test]
fn delete_timeout_keeps_config_and_target_change_requires_reconfirmation() {
    let (entered, waiting) = mpsc::channel();
    let (resume, release) = mpsc::channel();
    let e = Arc::new(WorkspaceEngine::with_boundary(Box::new(Boundary {
        entered,
        resume: release,
    })));
    seed(&e);
    for success in [false, true] {
        let worker = e.clone();
        let handle = std::thread::spawn(move || {
            worker.execute(command(
                serde_json::json!({"kind":"deleteEndpoint","id":"e"}),
            ))
        });
        waiting
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        if success {
            let mut endpoint = e.query().unwrap().catalog.endpoints[0].clone();
            endpoint.name = "changed".into();
            e.execute(command(
                serde_json::json!({"kind":"saveEndpoint","endpoint":endpoint}),
            ))
            .unwrap();
        }
        resume.send(success).unwrap();
        let error = handle.join().unwrap().unwrap_err();
        assert_eq!(
            error.code,
            if success {
                "CONFIG_CHANGED"
            } else {
                "RELEASE_TIMEOUT"
            }
        );
        let view = e.query().unwrap();
        assert_eq!(view.catalog.endpoints.len(), 1);
        assert_eq!(
            view.application_states[&EndpointId("e".into())],
            if success {
                ApplyState::Stopped
            } else {
                ApplyState::Failed
            }
        );
    }
}
