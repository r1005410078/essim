#![cfg(feature = "tcp-runtime")]
//! 真实端口生命周期；独立协议兼容性由PyModbus验证脚本覆盖。
use essim_core::{
    application::commands::{batch_targets::BatchRequest, Action, Command},
    domain::identity::EndpointId,
};
use essim_infrastructure::{
    batch_lifecycle::{BatchCoordinator, DraftDisposition},
    workspace::WorkspaceEngine,
};
use std::{
    net::{TcpListener, TcpStream},
    sync::Arc,
    time::{Duration, Instant},
};
fn command(e: &WorkspaceEngine, action: Action) {
    e.execute(Command {
        command_id: format!("cmd-{:?}", Instant::now()),
        action,
    })
    .unwrap();
}
fn setup(e: &WorkspaceEngine, id: &str, port: u16) {
    command(e,Action::SaveModel{model:serde_json::from_value(serde_json::json!({"id":format!("m-{id}"),"name":"M","points":[{"id":format!("p-{id}"),"name":"P","category":"measurement","area":"holding","address":0,"valueType":"u16","scale":"1","unit":"","initial":{"kind":"unsigned","value":"7"},"initialEngineering":"7","byteOrder":"big","wordOrder":"big","note":"","slaveWritable":true,"masterPeriodMs":1000}]})).unwrap()});
    command(e,Action::CreateEndpoint{endpoint:serde_json::from_value(serde_json::json!({"id":id,"name":id,"role":"slave","transport":{"kind":"tcp","address":"127.0.0.1","port":port},"units":[{"id":format!("u-{id}"),"name":"U","station":1,"enabled":true,"model":{"id":format!("m-{id}")},"device":null}]})).unwrap()});
}
fn request(id: &str, op: &str) -> BatchRequest {
    serde_json::from_value(
        serde_json::json!({"id":id,"operation":op,"scope":{"kind":"all"},"concurrency":2}),
    )
    .unwrap()
}
fn finish(b: &BatchCoordinator) -> essim_core::application::queries::batch::BatchView {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let v = b.query().unwrap().unwrap();
        if v.finished {
            return v;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn real_apply_start_partial_failure_retry_and_stop_release_ports() {
    let root = std::env::temp_dir().join(format!(
        "essim-batch-{}-{:?}",
        std::process::id(),
        Instant::now()
    ));
    let e = Arc::new(WorkspaceEngine::with_tcp_runtime(root.clone()).unwrap());
    let occupied = TcpListener::bind("127.0.0.1:0").unwrap();
    let bad = occupied.local_addr().unwrap().port();
    let good = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    setup(&e, "a", good);
    setup(&e, "b", bad);
    let b = BatchCoordinator::new(e.clone()).unwrap();
    b.preview(request("apply", "apply"), DraftDisposition::default())
        .unwrap();
    b.start("apply").unwrap();
    assert_eq!(finish(&b).counts.succeeded, 2);
    b.preview(request("start", "start"), DraftDisposition::default())
        .unwrap();
    b.start("start").unwrap();
    let v = finish(&b);
    assert_eq!(v.counts.succeeded, 1);
    assert_eq!(v.counts.failed, 1);
    assert!(TcpStream::connect(("127.0.0.1", good)).is_ok());
    drop(occupied);
    let retry = b
        .retry("start", "retry".into(), DraftDisposition::default())
        .unwrap();
    assert_eq!(retry.plan.targets.len(), 1);
    b.start("retry").unwrap();
    assert_eq!(finish(&b).counts.succeeded, 1);
    b.preview(request("again", "start"), DraftDisposition::default())
        .unwrap();
    b.start("again").unwrap();
    assert_eq!(finish(&b).counts.skipped, 2);
    b.preview(request("stop", "stop"), DraftDisposition::default())
        .unwrap();
    b.start("stop").unwrap();
    assert_eq!(finish(&b).counts.succeeded, 2);
    let _a = TcpListener::bind(("127.0.0.1", good)).unwrap();
    let _b = TcpListener::bind(("127.0.0.1", bad)).unwrap();
    assert_eq!(e.query().unwrap().catalog.applied.len(), 2);
    drop(b);
    e.shutdown_slave();
    let _ = std::fs::remove_dir_all(root);
}
#[test]
fn unsaved_and_retained_write_drafts_skip_and_changed_plan_does_not_apply() {
    let root = std::env::temp_dir().join(format!(
        "essim-batch-drafts-{}-{:?}",
        std::process::id(),
        Instant::now()
    ));
    let e = Arc::new(WorkspaceEngine::with_tcp_runtime(root.clone()).unwrap());
    setup(&e, "a", 18001);
    setup(&e, "b", 18002);
    let b = BatchCoordinator::new(e.clone()).unwrap();
    let preview = b
        .preview(
            request("drafts", "apply"),
            DraftDisposition {
                unsaved: vec![EndpointId("a".into())],
                retain_writes: vec![EndpointId("b".into())],
            },
        )
        .unwrap();
    assert!(preview.items.iter().all(|i| i.reason.is_some()));
    b.start("drafts").unwrap();
    assert_eq!(finish(&b).counts.skipped, 2);
    assert!(e.query().unwrap().catalog.applied.is_empty());
    b.preview(request("changed", "apply"), DraftDisposition::default())
        .unwrap();
    let mut a = e.query().unwrap().catalog.endpoints[0].clone();
    a.name = "changed".into();
    command(&e, Action::SaveEndpoint { endpoint: a });
    b.start("changed").unwrap();
    let v = finish(&b);
    assert_eq!(v.counts.skipped, 1);
    assert_eq!(v.counts.succeeded, 1);
    drop(b);
    e.shutdown_slave();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn master_batch_start_connects_without_polling_and_stop_closes_connection() {
    let root = std::env::temp_dir().join(format!(
        "essim-batch-master-{}-{:?}",
        std::process::id(),
        Instant::now()
    ));
    let e = Arc::new(WorkspaceEngine::with_tcp_runtime(root.clone()).unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    setup(&e, "master", port);
    let mut endpoint = e.query().unwrap().catalog.endpoints[0].clone();
    endpoint.role = essim_core::domain::endpoint::Role::Master;
    command(
        &e,
        Action::DeleteEndpoint {
            id: endpoint.id.clone(),
        },
    );
    command(&e, Action::CreateEndpoint { endpoint });
    let b = BatchCoordinator::new(e.clone()).unwrap();
    b.preview(request("apply", "apply"), DraftDisposition::default())
        .unwrap();
    b.start("apply").unwrap();
    assert_eq!(finish(&b).counts.succeeded, 1);
    b.preview(request("connect", "start"), DraftDisposition::default())
        .unwrap();
    b.start("connect").unwrap();
    assert_eq!(finish(&b).counts.succeeded, 1);
    let (mut socket, _) = listener.accept().unwrap();
    socket
        .set_read_timeout(Some(Duration::from_millis(30)))
        .unwrap();
    let mut buffer = [0; 1];
    use std::io::Read;
    assert!(socket.read(&mut buffer).is_err());
    let q = e
        .query_master(essim_core::application::queries::master::MasterQuery {
            endpoint: EndpointId("master".into()),
            unit: None,
            points: vec![],
        })
        .unwrap();
    assert!(!q.polling);
    assert_eq!(q.requests, 0);
    b.preview(request("disconnect", "stop"), DraftDisposition::default())
        .unwrap();
    b.start("disconnect").unwrap();
    assert_eq!(finish(&b).counts.succeeded, 1);
    assert_eq!(socket.read(&mut buffer).unwrap(), 0);
    drop(b);
    e.shutdown_slave();
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn rtu_batch_stop_releases_exclusive_serial_path() {
    use serialport::SerialPort as _;
    let root = std::env::temp_dir().join(format!(
        "essim-batch-rtu-{}-{:?}",
        std::process::id(),
        Instant::now()
    ));
    let e = Arc::new(WorkspaceEngine::with_tcp_runtime(root.clone()).unwrap());
    let (_wire, slave) = serialport::TTYPort::pair().unwrap();
    let path = slave.name().unwrap();
    setup(&e, "rtu", 19000);
    let mut endpoint = e.query().unwrap().catalog.endpoints[0].clone();
    endpoint.transport=serde_json::from_value(serde_json::json!({"kind":"rtu","path":path,"baud":19200,"parity":"none","dataBits":8,"stopBits":2})).unwrap();
    command(
        &e,
        Action::DeleteEndpoint {
            id: endpoint.id.clone(),
        },
    );
    command(&e, Action::CreateEndpoint { endpoint });
    let b = BatchCoordinator::new(e.clone()).unwrap();
    for (id, op) in [("apply", "apply"), ("start", "start"), ("stop", "stop")] {
        b.preview(request(id, op), DraftDisposition::default())
            .unwrap();
        b.start(id).unwrap();
        assert_eq!(finish(&b).counts.succeeded, 1);
    }
    let config = libmodbus_rs::RtuConfig::new(&path, 19200, 'N', 8, 2, 1, 50, 2).unwrap();
    let reopened = libmodbus_rs::SerialPort::open(&path, &config).unwrap();
    drop(reopened);
    drop(b);
    e.shutdown_slave();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn exhausted_runtime_budget_refuses_new_start_keeps_running_and_stop_available() {
    use essim_infrastructure::{
        operation_gates::OperationGates,
        resources::{ResourceManager, ResourcePolicy},
    };
    let root = std::env::temp_dir().join(format!(
        "essim-batch-budget-{}-{:?}",
        std::process::id(),
        Instant::now()
    ));
    let manager = ResourceManager::with_policy(ResourcePolicy {
        limit: essim_core::domain::resources::ResourceUsage {
            endpoints: 1,
            ..ResourcePolicy::default().limit
        },
        ..ResourcePolicy::default()
    })
    .unwrap();
    let e = Arc::new(
        WorkspaceEngine::with_runtime_resources(
            root.clone(),
            OperationGates::default(),
            manager.clone(),
        )
        .unwrap(),
    );
    let good = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let other = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    setup(&e, "a", good);
    setup(&e, "b", other);
    let b = BatchCoordinator::new(e.clone()).unwrap();
    b.preview(request("apply", "apply"), DraftDisposition::default())
        .unwrap();
    b.start("apply").unwrap();
    assert_eq!(finish(&b).counts.succeeded, 2);
    b.preview(request("start", "start"), DraftDisposition::default())
        .unwrap();
    b.start("start").unwrap();
    let v = finish(&b);
    assert_eq!(v.counts.succeeded, 1);
    assert_eq!(v.counts.failed, 1);
    let q = manager.query().unwrap();
    assert_eq!(q.current.endpoints, 1);
    assert_eq!(q.reserved.endpoints, 0);
    assert!(
        TcpStream::connect(("127.0.0.1", good)).is_ok()
            || TcpStream::connect(("127.0.0.1", other)).is_ok()
    );
    b.preview(request("stop", "stop"), DraftDisposition::default())
        .unwrap();
    b.start("stop").unwrap();
    finish(&b);
    assert_eq!(manager.query().unwrap().current.endpoints, 0);
    drop(b);
    e.shutdown_slave();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn navigation_and_task_summaries_do_not_grow_with_model_points() {
    let e = WorkspaceEngine::default();
    setup(&e, "a", 21000);
    let before = serde_json::to_vec(&e.query_navigation().unwrap())
        .unwrap()
        .len();
    let mut model = e.query().unwrap().catalog.models[0].clone();
    let point = model.points[0].clone();
    model.points = (0..10000)
        .map(|i| {
            let mut p = point.clone();
            p.id = essim_core::domain::identity::PointId(format!("point-{i}"));
            p.address = i;
            p
        })
        .collect();
    command(&e, Action::SaveModel { model });
    let after = e.query_navigation().unwrap();
    let size = serde_json::to_vec(&after).unwrap().len();
    assert!(size <= before + 10);
    assert!(after.catalog.models[0].points.is_empty());
    assert!(after.catalog.values.is_empty());
    assert_eq!(
        e.load_model(&essim_core::domain::identity::ModelId("m-a".into()))
            .unwrap()
            .points
            .len(),
        10000
    );
}

#[test]
fn slow_record_file_does_not_block_same_protocol_worker_or_cached_queries() {
    use essim_infrastructure::{
        operation_gates::OperationGates,
        recording::{DurableSink, LazyFile, RecordingPool, SinkFactory},
        resources::ResourceManager,
    };
    use std::io::{self, Write};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Condvar, Mutex,
    };
    #[derive(Default)]
    struct Control {
        released: Mutex<bool>,
        wake: Condvar,
        entered: AtomicUsize,
        files: AtomicUsize,
    }
    struct Slow {
        file: LazyFile,
        control: Arc<Control>,
        delay: bool,
    }
    impl Write for Slow {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.file.write(b)
        }
        fn flush(&mut self) -> io::Result<()> {
            self.file.flush()
        }
    }
    impl DurableSink for Slow {
        fn sync_data(&mut self) -> io::Result<()> {
            if self.delay {
                self.control.entered.fetch_add(1, Ordering::SeqCst);
                let mut r = self.control.released.lock().unwrap();
                while !*r {
                    r = self.control.wake.wait(r).unwrap();
                }
            }
            self.file.sync_data()
        }
    }
    struct Factory(Arc<Control>);
    impl SinkFactory for Factory {
        fn create(&self, path: std::path::PathBuf) -> Box<dyn DurableSink> {
            Box::new(Slow {
                file: LazyFile::new(path),
                control: self.0.clone(),
                delay: self.0.files.fetch_add(1, Ordering::SeqCst) == 0,
            })
        }
    }
    struct Release(Arc<Control>);
    impl Drop for Release {
        fn drop(&mut self) {
            *self.0.released.lock().unwrap() = true;
            self.0.wake.notify_all();
        }
    }
    let root = std::env::temp_dir().join(format!(
        "essim-batch-disk-{}-{:?}",
        std::process::id(),
        Instant::now()
    ));
    let control = Arc::new(Control::default());
    let manager = ResourceManager::default();
    let pool =
        RecordingPool::with_factory(manager.clone(), Arc::new(Factory(control.clone()))).unwrap();
    let e = WorkspaceEngine::with_runtime_recording(
        root.clone(),
        OperationGates::default(),
        manager,
        pool.clone(),
    )
    .unwrap();
    let _release = Release(control.clone());
    let pa = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let pe = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    // a与e在4个协议线程时哈希到同一线程，1～3线程环境下仍验证固定调度不中断。
    setup(&e, "a", pa);
    setup(&e, "e", pe);
    for id in ["a", "e"] {
        command(
            &e,
            Action::Apply {
                id: EndpointId(id.into()),
                reset_incompatible: false,
            },
        );
    }
    let gates = e.operation_gates();
    let permit = gates.acquire(&EndpointId("a".into())).unwrap();
    let mut completion = e
        .submit_slave_permitted(
            essim_core::application::commands::slave::SlaveCommand {
                command_id: "slow".into(),
                endpoint: EndpointId("a".into()),
                action: essim_core::application::commands::slave::SlaveAction::Start {
                    max_clients: 10,
                },
            },
            &permit,
        )
        .unwrap();
    let until = Instant::now() + Duration::from_secs(3);
    while control.entered.load(Ordering::SeqCst) == 0 {
        assert!(Instant::now() < until);
        std::thread::yield_now();
    }
    assert!(completion
        .wait_timeout(Duration::from_millis(1))
        .unwrap()
        .is_none());
    let now = Instant::now();
    let q = e
        .query_slave(essim_core::application::queries::slave::SlaveQuery {
            endpoint: EndpointId("a".into()),
            unit: None,
            points: vec![],
        })
        .unwrap();
    assert!(now.elapsed() < Duration::from_millis(100));
    assert!(q.pending_recording);
    assert_eq!(q.last_reliable, "0");
    let healthy = e
        .execute_slave(essim_core::application::commands::slave::SlaveCommand {
            command_id: "healthy".into(),
            endpoint: EndpointId("e".into()),
            action: essim_core::application::commands::slave::SlaveAction::Start {
                max_clients: 10,
            },
        })
        .unwrap();
    assert_eq!(
        healthy.state,
        essim_core::application::queries::slave::SlaveState::Running
    );
    assert!(TcpStream::connect(("127.0.0.1", pe)).is_ok());
    assert!(gates.acquire(&EndpointId("a".into())).is_err());
    drop(_release);
    assert!(completion
        .wait_timeout(Duration::from_secs(3))
        .unwrap()
        .unwrap()
        .is_ok());
    drop(permit);
    e.shutdown_slave();
    assert_eq!(pool.query().unwrap().queued_bytes, 0);
    let _ = std::fs::remove_dir_all(root);
}
