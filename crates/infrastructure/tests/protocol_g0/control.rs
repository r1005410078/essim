use essim_core::domain::identity::EndpointId;
use essim_infrastructure::operation_gates::OperationGates;
use std::{
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

// 生命周期凭证只能由此设施派生；与真实业务门使用不同准入路径。
struct LifecycleLease {
    owner: Rc<()>,
    endpoint: EndpointId,
    generation: u64,
}
struct Control {
    owner: Rc<()>,
    endpoint: EndpointId,
    generation: u64,
    gates: OperationGates,
    stopping: AtomicBool,
}
impl Control {
    fn new(endpoint: &str) -> Self {
        Self {
            owner: Rc::new(()),
            endpoint: EndpointId(endpoint.into()),
            generation: 1,
            gates: OperationGates::default(),
            stopping: AtomicBool::new(false),
        }
    }
    fn lifecycle(&self) -> LifecycleLease {
        LifecycleLease {
            owner: self.owner.clone(),
            endpoint: self.endpoint.clone(),
            generation: self.generation,
        }
    }
    fn stop(&self, lease: &LifecycleLease) -> Result<(), &'static str> {
        if !Rc::ptr_eq(&lease.owner, &self.owner)
            || lease.endpoint != self.endpoint
            || lease.generation != self.generation
        {
            return Err("LifecycleContextMismatch");
        }
        self.stopping.store(true, Ordering::Release);
        Ok(())
    }
    fn admit_data(&self) -> Result<(), &'static str> {
        if self.stopping.load(Ordering::Acquire) {
            return Err("Stopping");
        }
        self.gates
            .acquire(&self.endpoint)
            .map(drop)
            .map_err(|_| "Busy")
    }
}
#[test]
fn timed_out_held_permit_and_full_queue_do_not_block_trusted_stop() {
    let control = Control::new("endpoint");
    let permit = control.gates.acquire(&control.endpoint).unwrap();
    let (sender, receiver) = mpsc::sync_channel(1);
    let (reply, wait) = mpsc::sync_channel::<()>(1);
    sender.try_send((permit, reply)).unwrap(); // 实际所有者／队列持有凭证和同一次操作的完成回复。
    assert!(matches!(
        wait.recv_timeout(Duration::from_millis(1)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    let (other_reply, _other_wait) = mpsc::sync_channel(1);
    assert!(sender
        .try_send((
            control.gates.acquire(&EndpointId("other".into())).unwrap(),
            other_reply
        ))
        .is_err());
    assert_eq!(control.admit_data(), Err("Busy"));
    assert_eq!(
        control.gates.acquire(&control.endpoint).err().unwrap().code,
        "ENDPOINT_BUSY"
    ); // 旧stop共用业务门的反例。
    let lease = control.lifecycle();
    control.stop(&lease).unwrap();
    control.stop(&lease).unwrap(); // 应用内部复用，不重入申请门。
    assert_eq!(control.admit_data(), Err("Stopping"));
    assert!(control.gates.ensure_available(&control.endpoint).is_err()); // stop没有夺走原许可。
    let (owned_permit, reply) = receiver.recv().unwrap();
    assert!(control.gates.ensure_available(&control.endpoint).is_err());
    drop(owned_permit);
    reply.send(()).unwrap();
    wait.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(control.gates.ensure_available(&control.endpoint).is_ok());
}
#[test]
fn lifecycle_rejects_other_project_endpoint_and_stale_generation() {
    let mut a = Control::new("e");
    let b = Control::new("e");
    assert_eq!(a.stop(&b.lifecycle()), Err("LifecycleContextMismatch"));
    let wrong_endpoint = LifecycleLease {
        endpoint: EndpointId("other".into()),
        ..a.lifecycle()
    };
    assert!(a.stop(&wrong_endpoint).is_err());
    let old = a.lifecycle();
    a.generation += 1;
    assert!(a.stop(&old).is_err());
    assert!(!a.stopping.load(Ordering::Acquire));
}
