#![cfg(feature = "tcp-runtime")]
//! TCP从站真实socket回归；独立主站互操作证据另行记录。
use essim_core::{
    application::{
        commands::{
            slave::{SlaveAction, SlaveCommand},
            Action, Command,
        },
        queries::slave::{SlaveQuery, SlaveState},
    },
    domain::{
        identity::{EndpointId, PointId, UnitId},
        point_table::RawValue,
    },
};
use essim_infrastructure::workspace::WorkspaceEngine;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
static NEXT_ROOT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
fn root() -> PathBuf {
    std::env::temp_dir().join(format!(
        "essim-m04-{}-{}-{}",
        std::process::id(),
        NEXT_ROOT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
fn configure(engine: &WorkspaceEngine, id: &str, port: u16) {
    let point = |id: &str,
                 area: &str,
                 address: u16,
                 kind: &str,
                 writable: bool,
                 initial: serde_json::Value| serde_json::json!({"id":id,"name":id,"category":"measurement","area":area,"address":address,"valueType":kind,"scale":"1","unit":"","initial":initial,"initialEngineering":initial["value"].as_str().map(str::to_owned).unwrap_or_else(||initial["value"].to_string()),"byteOrder":"big","wordOrder":"big","note":"","slaveWritable":writable,"masterPeriodMs":1000});
    let model = serde_json::json!({"id":format!("m-{id}"),"name":"M","points":[point("h","holding",10,"u16",true,serde_json::json!({"kind":"unsigned","value":"7"})),point("ro","holding",12,"u16",false,serde_json::json!({"kind":"unsigned","value":"9"})),point("c","coil",0,"bool",true,serde_json::json!({"kind":"bool","value":true})),point("d","discrete",0,"bool",false,serde_json::json!({"kind":"bool","value":false})),point("i","input",0,"u16",false,serde_json::json!({"kind":"unsigned","value":"33"}))]});
    for action in [
        serde_json::json!({"kind":"saveModel","model":model}),
        serde_json::json!({"kind":"createEndpoint","endpoint":{"id":id,"name":id,"role":"slave","transport":{"kind":"tcp","address":"127.0.0.1","port":port},"units":[{"id":format!("{id}-u1"),"name":"U1","station":1,"enabled":true,"model":{"id":format!("m-{id}")},"device":null},{"id":format!("{id}-u2"),"name":"U2","station":2,"enabled":true,"model":{"id":format!("m-{id}")},"device":null}]}}),
        serde_json::json!({"kind":"apply","id":id,"resetIncompatible":false}),
    ] {
        let action: Action = serde_json::from_value(action).unwrap();
        if let Action::CreateEndpoint { endpoint } = &action {
            let mut first = endpoint.clone();
            first.units.truncate(1);
            engine
                .execute(Command {
                    command_id: "create".into(),
                    action: Action::CreateEndpoint { endpoint: first },
                })
                .unwrap();
            engine
                .execute(Command {
                    command_id: "save".into(),
                    action: Action::SaveEndpoint {
                        endpoint: endpoint.clone(),
                    },
                })
                .unwrap();
        } else {
            engine
                .execute(Command {
                    command_id: "setup".into(),
                    action,
                })
                .unwrap();
        }
    }
}
fn run(
    engine: &WorkspaceEngine,
    id: &str,
    action: SlaveAction,
) -> essim_core::application::queries::slave::SlaveView {
    engine
        .execute_slave(SlaveCommand {
            command_id: "test".into(),
            endpoint: EndpointId(id.into()),
            action,
        })
        .unwrap()
}
fn socket(port: u16) -> TcpStream {
    let stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream
}
fn frame(unit: u8, pdu: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0, 42, 0, 0, 0, (pdu.len() + 1) as u8, unit];
    bytes.extend_from_slice(pdu);
    bytes
}
fn read(stream: &mut TcpStream) -> Vec<u8> {
    let mut header = [0; 6];
    stream.read_exact(&mut header).unwrap();
    let mut bytes = vec![0; u16::from_be_bytes([header[4], header[5]]) as usize];
    stream.read_exact(&mut bytes).unwrap();
    bytes
}
fn request(stream: &mut TcpStream, unit: u8, pdu: &[u8]) -> Vec<u8> {
    stream.write_all(&frame(unit, pdu)).unwrap();
    read(stream)
}
#[test]
fn four_areas_permissions_unit_isolation_and_restart_preserve_values() {
    let path = root();
    let engine = WorkspaceEngine::with_tcp_runtime(path.clone()).unwrap();
    let port = free_port();
    configure(&engine, "e", port);
    run(&engine, "e", SlaveAction::Start { max_clients: 10 });
    let mut a = socket(port);
    let mut b = socket(port);
    assert_eq!(request(&mut a, 1, &[3, 0, 10, 0, 1]), [1, 3, 2, 0, 7]);
    assert_eq!(request(&mut a, 1, &[4, 0, 0, 0, 1]), [1, 4, 2, 0, 33]);
    assert_eq!(request(&mut a, 1, &[1, 0, 0, 0, 1]), [1, 1, 1, 1]);
    assert_eq!(request(&mut a, 1, &[2, 0, 0, 0, 1]), [1, 2, 1, 0]);
    assert_eq!(request(&mut a, 1, &[6, 0, 10, 0, 42]), [1, 6, 0, 10, 0, 42]);
    assert_eq!(request(&mut b, 1, &[3, 0, 10, 0, 1]), [1, 3, 2, 0, 42]);
    assert_eq!(request(&mut b, 2, &[3, 0, 10, 0, 1]), [2, 3, 2, 0, 7]);
    assert_eq!(request(&mut a, 1, &[5, 0, 0, 0, 0]), [1, 5, 0, 0, 0, 0]);
    assert_eq!(
        request(&mut a, 1, &[15, 0, 0, 0, 1, 1, 1]),
        [1, 15, 0, 0, 0, 1]
    );
    assert_eq!(
        request(&mut a, 1, &[16, 0, 10, 0, 1, 2, 0, 99]),
        [1, 16, 0, 10, 0, 1]
    );
    assert_eq!(request(&mut a, 1, &[6, 0, 12, 0, 22]), [1, 0x86, 2]);
    assert_eq!(
        request(&mut a, 1, &[16, 0, 10, 0, 3, 6, 0, 1, 0, 2, 0, 3]),
        [1, 0x90, 2]
    );
    assert_eq!(request(&mut a, 3, &[3, 0, 10, 0, 1]), [3, 0x83, 11]);
    assert_eq!(request(&mut a, 1, &[0x16, 0, 10, 0, 1]), [1, 0x96, 1]);
    let result = engine.execute_slave(SlaveCommand {
        command_id: "bad".into(),
        endpoint: EndpointId("e".into()),
        action: SlaveAction::SetValues {
            unit: UnitId("e-u1".into()),
            values: vec![
                (PointId("h".into()), RawValue::Unsigned(55)),
                (PointId("missing".into()), RawValue::Unsigned(1)),
            ],
        },
    });
    assert!(result.is_err());
    assert_eq!(request(&mut a, 1, &[3, 0, 10, 0, 1]), [1, 3, 2, 0, 99]);
    run(
        &engine,
        "e",
        SlaveAction::SetValues {
            unit: UnitId("e-u1".into()),
            values: vec![(PointId("h".into()), RawValue::Unsigned(77))],
        },
    );
    assert_eq!(request(&mut a, 1, &[3, 0, 10, 0, 1]), [1, 3, 2, 0, 77]);
    run(&engine, "e", SlaveAction::Stop);
    assert!(TcpListener::bind(("127.0.0.1", port)).is_ok());
    run(&engine, "e", SlaveAction::Start { max_clients: 10 });
    let mut a = socket(port);
    assert_eq!(request(&mut a, 1, &[3, 0, 10, 0, 1]), [1, 3, 2, 0, 77]);
    run(
        &engine,
        "e",
        SlaveAction::ResetValues {
            unit: UnitId("e-u1".into()),
            points: vec![PointId("h".into())],
        },
    );
    assert_eq!(request(&mut a, 1, &[3, 0, 10, 0, 1]), [1, 3, 2, 0, 7]);
    let view = run(&engine, "e", SlaveAction::Stop);
    assert_eq!(view.state, SlaveState::Stopped);
    let record = std::fs::read_to_string(view.record_path.unwrap()).unwrap();
    assert!(record.contains("snapshot"));
    assert!(record.contains("valuesCommitted"));
    drop(engine);
    let _ = std::fs::remove_dir_all(path);
}
#[test]
fn fragmented_combined_invalid_frames_and_slow_peer_are_isolated() {
    let path = root();
    let engine = WorkspaceEngine::with_tcp_runtime(path.clone()).unwrap();
    let port = free_port();
    configure(&engine, "e", port);
    run(&engine, "e", SlaveAction::Start { max_clients: 10 });
    let mut slow = socket(port);
    slow.write_all(&[0, 1, 0]).unwrap();
    let mut a = socket(port);
    let f = frame(1, &[3, 0, 10, 0, 1]);
    a.write_all(&f[..5]).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    a.write_all(&f[5..]).unwrap();
    assert_eq!(read(&mut a), [1, 3, 2, 0, 7]);
    let mut combined = f.clone();
    combined.extend_from_slice(&f);
    a.write_all(&combined).unwrap();
    assert_eq!(read(&mut a), [1, 3, 2, 0, 7]);
    assert_eq!(read(&mut a), [1, 3, 2, 0, 7]);
    let mut bad = socket(port);
    bad.write_all(&[0, 1, 0, 0, 0xff, 0xff, 1, 3]).unwrap();
    let mut byte = [0];
    assert!(matches!(bad.read(&mut byte), Ok(0) | Err(_)));
    assert_eq!(request(&mut a, 1, &[3, 0, 10, 0, 1]), [1, 3, 2, 0, 7]);
    run(&engine, "e", SlaveAction::Stop);
    drop(engine);
    let _ = std::fs::remove_dir_all(path);
}
#[test]
fn failed_bind_and_apply_release_preserve_actual_values() {
    let path = root();
    let engine = WorkspaceEngine::with_tcp_runtime(path.clone()).unwrap();
    let occupied = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = occupied.local_addr().unwrap().port();
    configure(&engine, "e", port);
    assert!(engine
        .execute_slave(SlaveCommand {
            command_id: "start".into(),
            endpoint: EndpointId("e".into()),
            action: SlaveAction::Start { max_clients: 10 }
        })
        .is_err());
    drop(occupied);
    run(&engine, "e", SlaveAction::Start { max_clients: 10 });
    let mut a = socket(port);
    request(&mut a, 1, &[6, 0, 10, 0, 88]);
    engine
        .execute(Command {
            command_id: "apply".into(),
            action: Action::Apply {
                id: EndpointId("e".into()),
                reset_incompatible: false,
            },
        })
        .unwrap();
    assert!(TcpListener::bind(("127.0.0.1", port)).is_ok());
    assert_eq!(
        engine.query().unwrap().catalog.values[&UnitId("e-u1".into())].raw[&PointId("h".into())],
        RawValue::Unsigned(88)
    );
    drop(engine);
    let _ = std::fs::remove_dir_all(path);
}
#[test]
fn recording_create_failure_never_opens_listener() {
    let path = root();
    std::fs::write(&path, b"not a directory").unwrap();
    let engine = WorkspaceEngine::with_tcp_runtime(path.clone()).unwrap();
    let port = free_port();
    configure(&engine, "e", port);
    assert!(engine
        .execute_slave(SlaveCommand {
            command_id: "start".into(),
            endpoint: EndpointId("e".into()),
            action: SlaveAction::Start { max_clients: 10 }
        })
        .is_err());
    assert!(TcpListener::bind(("127.0.0.1", port)).is_ok());
    assert_eq!(
        engine
            .query_slave(SlaveQuery {
                endpoint: EndpointId("e".into()),
                unit: None,
                points: vec![]
            })
            .unwrap()
            .state,
        SlaveState::Failed
    );
    drop(engine);
    let _ = std::fs::remove_file(path);
}

#[test]
fn client_limit_and_partial_frame_timeout_preserve_other_clients() {
    let path = root();
    let engine = WorkspaceEngine::with_tcp_runtime(path.clone()).unwrap();
    let port = free_port();
    configure(&engine, "e", port);
    run(&engine, "e", SlaveAction::Start { max_clients: 1 });
    let mut a = socket(port);
    assert_eq!(request(&mut a, 1, &[3, 0, 10, 0, 1]), [1, 3, 2, 0, 7]);
    let mut excess = socket(port);
    let _ = excess.write_all(&frame(1, &[3, 0, 10, 0, 1]));
    let mut byte = [0];
    assert!(matches!(excess.read(&mut byte), Ok(0) | Err(_)));
    assert_eq!(request(&mut a, 1, &[3, 0, 10, 0, 1]), [1, 3, 2, 0, 7]);
    drop(a);
    drop(excess);
    run(&engine, "e", SlaveAction::Stop);
    run(&engine, "e", SlaveAction::Start { max_clients: 2 });
    let mut slow = socket(port);
    slow.set_read_timeout(Some(Duration::from_secs(7))).unwrap();
    slow.write_all(&[0, 1, 0]).unwrap();
    let mut fast = socket(port);
    assert_eq!(request(&mut fast, 1, &[3, 0, 10, 0, 1]), [1, 3, 2, 0, 7]);
    assert_eq!(slow.read(&mut byte).unwrap(), 0);
    assert_eq!(request(&mut fast, 1, &[3, 0, 10, 0, 1]), [1, 3, 2, 0, 7]);
    run(&engine, "e", SlaveAction::Stop);
    drop(engine);
    let _ = std::fs::remove_dir_all(path);
}
#[test]
fn stopping_one_endpoint_does_not_stop_another_and_drop_releases_all() {
    let path = root();
    let port_a = free_port();
    let port_b = free_port();
    let engine = WorkspaceEngine::with_tcp_runtime(path.clone()).unwrap();
    configure(&engine, "a", port_a);
    configure(&engine, "b", port_b);
    run(&engine, "a", SlaveAction::Start { max_clients: 10 });
    run(&engine, "b", SlaveAction::Start { max_clients: 10 });
    let mut a = socket(port_a);
    let mut b = socket(port_b);
    assert_eq!(request(&mut a, 1, &[3, 0, 10, 0, 1]), [1, 3, 2, 0, 7]);
    run(&engine, "a", SlaveAction::Stop);
    assert_eq!(request(&mut b, 1, &[3, 0, 10, 0, 1]), [1, 3, 2, 0, 7]);
    drop(engine);
    assert!(TcpListener::bind(("127.0.0.1", port_a)).is_ok());
    assert!(TcpListener::bind(("127.0.0.1", port_b)).is_ok());
    let _ = std::fs::remove_dir_all(path);
}

#[test]
fn invalid_apply_is_rejected_before_stopping_the_live_endpoint() {
    let path = root();
    let engine = WorkspaceEngine::with_tcp_runtime(path.clone()).unwrap();
    let port = free_port();
    configure(&engine, "e", port);
    run(&engine, "e", SlaveAction::Start { max_clients: 10 });
    let mut socket = socket(port);
    let mut model = engine.query().unwrap().catalog.models[0].clone();
    let point = &mut model.points[0];
    point.value_type = essim_core::domain::point_table::ValueType::F32;
    point.initial = RawValue::Float(0.0);
    point.initial_engineering = "0".into();
    engine
        .execute(Command {
            command_id: "change-type".into(),
            action: Action::SaveModel { model },
        })
        .unwrap();
    let error = engine
        .execute(Command {
            command_id: "reject-apply".into(),
            action: Action::Apply {
                id: EndpointId("e".into()),
                reset_incompatible: false,
            },
        })
        .unwrap_err();
    assert_eq!(error.code, "INCOMPATIBLE_TYPE");
    assert_eq!(request(&mut socket, 1, &[3, 0, 10, 0, 1]), [1, 3, 2, 0, 7]);
    run(&engine, "e", SlaveAction::Stop);
    drop(engine);
    let _ = std::fs::remove_dir_all(path);
}

#[test]
fn deleting_saved_configuration_also_releases_units_still_in_the_applied_snapshot() {
    let path = root();
    let engine = WorkspaceEngine::with_tcp_runtime(path.clone()).unwrap();
    let port = free_port();
    configure(&engine, "e", port);
    run(&engine, "e", SlaveAction::Start { max_clients: 10 });
    let mut client = socket(port);
    assert_eq!(request(&mut client, 2, &[3, 0, 10, 0, 1]), [2, 3, 2, 0, 7]);
    let mut endpoint = engine.query().unwrap().catalog.endpoints[0].clone();
    endpoint.units.truncate(1);
    engine
        .execute(Command {
            command_id: "remove-draft-unit".into(),
            action: Action::SaveEndpoint { endpoint },
        })
        .unwrap();
    engine
        .execute(Command {
            command_id: "delete".into(),
            action: Action::DeleteEndpoint {
                id: EndpointId("e".into()),
            },
        })
        .unwrap();
    assert!(TcpListener::bind(("127.0.0.1", port)).is_ok());
    assert!(engine.query().unwrap().catalog.values.is_empty());
    drop(engine);
    let _ = std::fs::remove_dir_all(path);
}
#[test]
fn empty_applied_model_can_stop_and_restart_without_invented_values() {
    let path = root();
    let engine = WorkspaceEngine::with_tcp_runtime(path.clone()).unwrap();
    let port = free_port();
    configure(&engine, "e", port);
    let mut model = engine.query().unwrap().catalog.models[0].clone();
    model.points.clear();
    engine
        .execute(Command {
            command_id: "empty-model".into(),
            action: Action::SaveModel { model },
        })
        .unwrap();
    engine
        .execute(Command {
            command_id: "apply-empty".into(),
            action: Action::Apply {
                id: EndpointId("e".into()),
                reset_incompatible: false,
            },
        })
        .unwrap();
    for _ in 0..2 {
        run(&engine, "e", SlaveAction::Start { max_clients: 10 });
        let mut client = socket(port);
        assert_eq!(request(&mut client, 1, &[3, 0, 10, 0, 1]), [1, 0x83, 2]);
        run(&engine, "e", SlaveAction::Stop);
    }
    drop(engine);
    let _ = std::fs::remove_dir_all(path);
}

#[test]
fn observations_match_reliable_records_and_socket_bytes() {
    let root = root();
    let engine = WorkspaceEngine::with_tcp_runtime(root.clone()).unwrap();
    let port = free_port();
    configure(&engine, "observed", port);
    run(&engine, "observed", SlaveAction::Start { max_clients: 10 });
    let mut client = socket(port);
    let request = frame(1, &[3, 0, 10, 0, 1]);
    client.write_all(&request).unwrap();
    let response = read(&mut client);
    // 等待发送确认投影，避免把收到响应与可靠记录完成混为同一时刻。
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    let view = loop {
        let view = engine.query_slave(SlaveQuery {endpoint: EndpointId("observed".into()),unit:None,points:vec![]}).unwrap();
        if view.observations.iter().any(|row| row.event.contains("responseSent")) {break view;}
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    };
    let records: Vec<serde_json::Value> = std::fs::read_to_string(view.record_path.as_ref().unwrap()).unwrap().lines().map(|line|serde_json::from_str(line).unwrap()).collect();
    for observation in &view.observations {
        let record=records.iter().find(|r|r["sequence"]==observation.sequence).unwrap();
        assert_eq!(record["unixMs"],observation.unix_ms);
        assert_eq!(record["fact"],serde_json::from_str::<serde_json::Value>(&observation.event).unwrap());
    }
    let bytes = |kind:&str| -> Vec<u8> { view.observations.iter().filter_map(|row|serde_json::from_str::<serde_json::Value>(&row.event).ok()).find(|fact|fact["kind"]==kind).unwrap()["bytes"].as_array().unwrap().iter().map(|v|v.as_u64().unwrap() as u8).collect() };
    assert_eq!(bytes("request"),request);
    let prepared=bytes("responsePrepared");
    assert_eq!(prepared[..6], [0,42,0,0,0,response.len() as u8]);
    assert_eq!(prepared[6..],response);
    run(&engine,"observed",SlaveAction::Stop);
    drop(engine);
    std::fs::remove_dir_all(root).unwrap();
}
