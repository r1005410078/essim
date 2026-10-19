#![cfg(feature = "tcp-runtime")]
//! 协议夹具验证主站运行语义；独立PyModbus互操作另行记录。
use essim_core::{
    application::{
        commands::master::{MasterAction, MasterCommand},
        queries::master::{MasterQuery, MasterView},
    },
    domain::{
        identity::{EndpointId, PointId, UnitId},
        point_table::RawValue,
    },
};
use essim_infrastructure::tcp_master::{MasterSnapshot, TcpMasterRuntime};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
fn snapshot(port: u16) -> MasterSnapshot {
    serde_json::from_value(serde_json::json!({"endpoint":{"id":"e","name":"M","role":"master","master":{"requestTimeoutMs":100,"connectTimeoutMs":100,"reconnectMs":100,"staleMultiplier":1},"transport":{"kind":"tcp","address":"127.0.0.1","port":port},"units":[{"id":"u","name":"U","station":1,"enabled":true,"model":{"id":"m"},"device":null}]},"values":{"u":{"model":{"id":"m","name":"M","points":(0..3).map(|i|serde_json::json!({"id":format!("p{i}"),"name":"P","category":"adjustment","area":"holding","address":i*2,"valueType":"u16","scale":"1","unit":"","initial":{"kind":"unsigned","value":"0"},"initialEngineering":"0","byteOrder":"big","wordOrder":"big","note":"","slaveWritable":false,"masterPeriodMs":50})).collect::<Vec<_>>()},"raw":{},"stale":true}}})).unwrap()
}
fn runtime() -> TcpMasterRuntime {
    let root = std::env::temp_dir().join(format!(
        "master-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    TcpMasterRuntime::new(root)
}
fn command(r: &TcpMasterRuntime, action: MasterAction, s: Option<MasterSnapshot>) -> MasterView {
    r.execute(
        MasterCommand {
            command_id: "test".into(),
            endpoint: EndpointId("e".into()),
            action,
        },
        s,
    )
    .unwrap()
}
fn query(r: &TcpMasterRuntime) -> MasterView {
    r.query(MasterQuery {
        endpoint: EndpointId("e".into()),
        unit: Some(UnitId("u".into())),
        points: vec![
            PointId("p0".into()),
            PointId("p1".into()),
            PointId("p2".into()),
        ],
    })
    .unwrap()
}
fn wait(r: &TcpMasterRuntime, predicate: impl Fn(&MasterView) -> bool) -> MasterView {
    let end = Instant::now() + Duration::from_secs(3);
    loop {
        let v = query(r);
        if predicate(&v) {
            return v;
        }
        assert!(Instant::now() < end, "未达预期：{v:?}");
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn server(mode: usize) -> (u16, Arc<AtomicUsize>, std::thread::JoinHandle<()>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let count = Arc::new(AtomicUsize::new(0));
    let n = count.clone();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = l.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut value = 7u16;
        loop {
            let mut header = [0; 6];
            if stream.read_exact(&mut header).is_err() {
                break;
            }
            let len = u16::from_be_bytes([header[4], header[5]]) as usize;
            let mut pdu = vec![0; len];
            if stream.read_exact(&mut pdu).is_err() {
                break;
            }
            n.fetch_add(1, Ordering::Relaxed);
            let fc = pdu[1];
            if mode == 1 && fc == 6 {
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
            if mode == 3 && fc == 6 {
                std::thread::sleep(Duration::from_millis(60));
            }
            let address = u16::from_be_bytes([pdu[2], pdu[3]]);
            let body = if fc == 3 {
                if (mode == 2 && address == 2)
                    || (mode == 4 && address == 0 && n.load(Ordering::Relaxed) > 1)
                {
                    vec![pdu[0], 0x83, 2]
                } else {
                    vec![pdu[0], 3, 2, (value >> 8) as u8, value as u8]
                }
            } else {
                value = u16::from_be_bytes([pdu[4], pdu[5]]);
                pdu.clone()
            };
            let mut response = header[..4].to_vec();
            response.extend((body.len() as u16).to_be_bytes());
            response.extend(body);
            if stream.write_all(&response).is_err() {
                break;
            }
        }
    });
    (port, count, handle)
}
#[test]
fn read_write_readback_trace_and_quality() {
    let (port, _, server) = server(0);
    let r = runtime();
    command(&r, MasterAction::Connect, Some(snapshot(port)));
    wait(&r, |v| !v.busy && v.state == "connected");
    command(
        &r,
        MasterAction::Read {
            unit: UnitId("u".into()),
            points: vec![],
        },
        None,
    );
    let v = wait(&r, |v| !v.busy);
    assert_eq!(
        v.points[&PointId("p0".into())].value,
        Some(RawValue::Unsigned(7))
    );
    command(
        &r,
        MasterAction::Write {
            unit: UnitId("u".into()),
            values: vec![(PointId("p0".into()), RawValue::Unsigned(42))],
        },
        None,
    );
    let v = wait(&r, |v| !v.busy);
    assert_eq!(
        v.points[&PointId("p0".into())].write_result.as_deref(),
        Some("协议已确认，回验一致")
    );
    assert!(v.observations.iter().any(|o| o.event.contains("frame")));
    command(
        &r,
        MasterAction::Write {
            unit: UnitId("u".into()),
            values: vec![(PointId("p0".into()), RawValue::Unsigned(999999))],
        },
        None,
    );
    let rejected = wait(&r, |v| !v.busy);
    assert_eq!(
        rejected.points[&PointId("p0".into())]
            .write_result
            .as_deref(),
        Some("校验未通过，未发送")
    );
    assert_eq!(
        rejected.points[&PointId("p0".into())].value,
        Some(RawValue::Unsigned(42))
    );

    std::thread::sleep(Duration::from_millis(80));
    assert_eq!(query(&r).points[&PointId("p0".into())].quality, "stale");
    r.disconnect_wait(&EndpointId("e".into())).unwrap();
    r.shutdown();
    server.join().unwrap();
}
#[test]
fn write_timeout_never_retries_and_skips_remaining() {
    let (port, count, server) = server(1);
    let r = runtime();
    command(&r, MasterAction::Connect, Some(snapshot(port)));
    wait(&r, |v| !v.busy && v.state == "connected");
    command(
        &r,
        MasterAction::Write {
            unit: UnitId("u".into()),
            values: vec![
                (PointId("p0".into()), RawValue::Unsigned(42)),
                (PointId("p1".into()), RawValue::Unsigned(43)),
            ],
        },
        None,
    );
    let v = wait(&r, |v| !v.busy);
    assert!(v.points[&PointId("p0".into())]
        .write_result
        .as_deref()
        .unwrap()
        .contains("不确定"));
    assert_eq!(
        v.points[&PointId("p1".into())].write_result.as_deref(),
        Some("未执行")
    );
    std::thread::sleep(Duration::from_millis(250));
    assert_eq!(count.load(Ordering::Relaxed), 1);
    r.shutdown();
    server.join().unwrap();
}
#[test]
fn read_failure_keeps_last_value_and_prevalidation_writes_nothing() {
    let (port, count, server) = server(2);
    let r = runtime();
    command(&r, MasterAction::Connect, Some(snapshot(port)));
    wait(&r, |v| !v.busy && v.state == "connected");
    command(
        &r,
        MasterAction::Read {
            unit: UnitId("u".into()),
            points: vec![],
        },
        None,
    );
    let v = wait(&r, |v| !v.busy);
    assert_eq!(
        v.points[&PointId("p0".into())].value,
        Some(RawValue::Unsigned(7))
    );
    assert_eq!(v.points[&PointId("p1".into())].quality, "failed");
    let before = count.load(Ordering::Relaxed);
    command(
        &r,
        MasterAction::Write {
            unit: UnitId("u".into()),
            values: vec![
                (PointId("p0".into()), RawValue::Unsigned(42)),
                (PointId("p2".into()), RawValue::Unsigned(999999)),
            ],
        },
        None,
    );
    wait(&r, |v| !v.busy);
    assert_eq!(count.load(Ordering::Relaxed), before);
    r.shutdown();
    server.join().unwrap();
}

#[test]
fn cancellation_after_sent_write_keeps_confirmation_and_does_not_send_rest() {
    let (port, count, server) = server(3);
    let r = runtime();
    command(&r, MasterAction::Connect, Some(snapshot(port)));
    wait(&r, |v| !v.busy && v.state == "connected");
    command(
        &r,
        MasterAction::Write {
            unit: UnitId("u".into()),
            values: vec![
                (PointId("p0".into()), RawValue::Unsigned(42)),
                (PointId("p1".into()), RawValue::Unsigned(43)),
            ],
        },
        None,
    );
    let deadline = Instant::now() + Duration::from_secs(1);
    while count.load(Ordering::Relaxed) == 0 {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    command(&r, MasterAction::Disconnect, None);
    let v = wait(&r, |v| !v.busy && v.state == "disconnected");
    assert_eq!(
        v.points[&PointId("p0".into())].write_result.as_deref(),
        Some("协议已确认，回验已取消")
    );
    assert_eq!(
        v.points[&PointId("p1".into())].write_result.as_deref(),
        Some("未执行")
    );
    assert_eq!(count.load(Ordering::Relaxed), 1);
    r.shutdown();
    server.join().unwrap();
}
#[test]
fn reapply_preserves_only_compatible_cached_points_and_stays_disconnected() {
    let (port, _, server) = server(0);
    let r = runtime();
    let s = snapshot(port);
    command(&r, MasterAction::Connect, Some(s.clone()));
    wait(&r, |v| !v.busy && v.state == "connected");
    command(
        &r,
        MasterAction::Read {
            unit: UnitId("u".into()),
            points: vec![],
        },
        None,
    );
    wait(&r, |v| !v.busy);
    r.disconnect_wait(&EndpointId("e".into())).unwrap();
    let mut next = s;
    next.values
        .get_mut(&UnitId("u".into()))
        .unwrap()
        .model
        .as_mut()
        .unwrap()
        .points[1]
        .address = 3;
    r.reconfigure(next).unwrap();
    let v = wait(&r, |v| !v.busy);
    assert_eq!(v.state, "disconnected");
    assert!(!v.polling);
    assert_eq!(
        v.points[&PointId("p0".into())].value,
        Some(RawValue::Unsigned(7))
    );
    assert_ne!(v.points[&PointId("p0".into())].quality, "valid");
    assert_eq!(v.points[&PointId("p1".into())].value, None);
    r.shutdown();
    server.join().unwrap();
}
#[test]
fn recording_creation_failure_prevents_connect() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let r = TcpMasterRuntime::new("/dev/null/not-a-directory".into());
    command(&r, MasterAction::Connect, Some(snapshot(port)));
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let v = r
            .query(MasterQuery {
                endpoint: EndpointId("e".into()),
                unit: None,
                points: vec![],
            })
            .unwrap();
        if !v.busy {
            assert!(v.error.is_some());
            assert_ne!(v.state, "connected");
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(listener.accept().is_err());
    r.shutdown();
}

#[test]
fn failed_read_of_same_point_preserves_success_value_and_time() {
    let (port, _, server) = server(4);
    let r = runtime();
    command(&r, MasterAction::Connect, Some(snapshot(port)));
    wait(&r, |v| !v.busy && v.state == "connected");
    let action = || MasterAction::Read {
        unit: UnitId("u".into()),
        points: vec![PointId("p0".into())],
    };
    command(&r, action(), None);
    let before = wait(&r, |v| !v.busy).points[&PointId("p0".into())].clone();
    command(&r, action(), None);
    let after = wait(&r, |v| !v.busy).points[&PointId("p0".into())].clone();
    assert_eq!(after.value, before.value);
    assert_eq!(after.unix_ms, before.unix_ms);
    assert_eq!(after.quality, "failed");
    assert!(after.error.is_some());
    r.shutdown();
    server.join().unwrap();
}
