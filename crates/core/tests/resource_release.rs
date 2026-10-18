//! 受控资源所有者验证请求与确认分离；不构成操作系统占用或真实通信证据。
use essim_core::{
    application::{resource_release::*, workspace::*},
    domain::foundation::*,
};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
#[derive(Default)]
struct Driver {
    states: BTreeMap<EndpointId, ApplyState>,
    requests: Vec<EndpointId>,
    timeout: bool,
    unconfirmed: bool,
}
struct Owner(Rc<RefCell<Driver>>);
impl ReleaseDriver for Owner {
    fn state(&self, id: &EndpointId) -> ApplyState {
        self.0
            .borrow()
            .states
            .get(id)
            .copied()
            .unwrap_or(ApplyState::Stopped)
    }
    fn request_stop(&mut self, e: &EndpointConfig) -> Result<(), Error> {
        let mut d = self.0.borrow_mut();
        d.requests.push(e.id.clone());
        d.states.insert(e.id.clone(), ApplyState::Releasing);
        Ok(())
    }
    fn wait_released(&mut self, id: &EndpointId, timeout_ms: u64) -> Result<(), Error> {
        assert_eq!(timeout_ms, 100);
        let mut d = self.0.borrow_mut();
        if d.timeout {
            return Err(Error::new("RELEASE_TIMEOUT", "超时尚未释放"));
        }
        if !d.unconfirmed {
            d.states.insert(id.clone(), ApplyState::Stopped);
        }
        Ok(())
    }
}
fn endpoint() -> EndpointConfig {
    serde_json::from_value(serde_json::json!({"id":"e","name":"E","role":"slave","transport":{"kind":"tcp","address":"127.0.0.1","port":1502},"units":[]})).unwrap()
}
#[test]
fn release_is_idempotent_and_timeout_does_not_claim_stopped_or_touch_others() {
    let e = endpoint();
    let d = Rc::new(RefCell::new(Driver::default()));
    d.borrow_mut()
        .states
        .insert(e.id.clone(), ApplyState::Running);
    d.borrow_mut()
        .states
        .insert(EndpointId("other".into()), ApplyState::Running);
    d.borrow_mut().timeout = true;
    let mut boundary = ConfirmedBoundary::new(Owner(d.clone()), 100);
    assert_eq!(boundary.stop(&e).unwrap_err().code, "RELEASE_TIMEOUT");
    assert_eq!(boundary.state(&e.id), ApplyState::Releasing);
    assert_eq!(
        d.borrow().states[&EndpointId("other".into())],
        ApplyState::Running
    );
    d.borrow_mut().timeout = false;
    boundary.stop(&e).unwrap();
    let count = d.borrow().requests.len();
    boundary.stop(&e).unwrap();
    assert_eq!(d.borrow().requests.len(), count);
}
#[test]
fn returning_success_without_stopped_state_is_rejected() {
    let e = endpoint();
    let d = Rc::new(RefCell::new(Driver {
        unconfirmed: true,
        ..Default::default()
    }));
    d.borrow_mut()
        .states
        .insert(e.id.clone(), ApplyState::Running);
    let mut b = ConfirmedBoundary::new(Owner(d), 100);
    assert_eq!(b.stop(&e).unwrap_err().code, "RELEASE_UNCONFIRMED");
}
