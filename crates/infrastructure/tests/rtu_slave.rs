#![cfg(all(feature = "tcp-runtime", unix))]
//! RTU资源与配置边界；独立协议互操作见Python脚本。
use essim_core::{
    application::{
        commands::{
            slave::{SlaveAction, SlaveCommand},
            Action, Command,
        },
        queries::slave::{SlaveQuery, SlaveState},
    },
    domain::identity::EndpointId,
};
use essim_infrastructure::workspace::WorkspaceEngine;
use serialport::SerialPort as _;
use std::io::{Read, Write};
fn action(engine: &WorkspaceEngine, value: serde_json::Value) {
    engine
        .execute(Command {
            command_id: "setup".into(),
            action: serde_json::from_value::<Action>(value).unwrap(),
        })
        .unwrap();
}
fn setup(engine: &WorkspaceEngine, id: &str, path: &str) {
    let model = serde_json::json!({"id":format!("m-{id}"),"name":"RTU","points":[{"id":"h","name":"h","category":"measurement","area":"holding","address":0,"valueType":"u16","scale":"1","unit":"","initial":{"kind":"unsigned","value":"7"},"initialEngineering":"7","byteOrder":"big","wordOrder":"big","note":"","slaveWritable":true,"masterPeriodMs":1000}]});
    action(
        engine,
        serde_json::json!({"kind":"saveModel","model":model}),
    );
    action(
        engine,
        serde_json::json!({"kind":"createEndpoint","endpoint":{"id":id,"name":id,"role":"slave","transport":{"kind":"rtu","path":path,"baud":19200,"parity":"none","dataBits":8,"stopBits":2},"units":[{"id":format!("{id}-u"),"name":"U","station":1,"enabled":true,"model":{"id":format!("m-{id}")},"device":null}]}}),
    );
    action(
        engine,
        serde_json::json!({"kind":"apply","id":id,"resetIncompatible":false}),
    );
}
fn command(id: &str, action: SlaveAction) -> SlaveCommand {
    SlaveCommand {
        command_id: "rtu-test".into(),
        endpoint: EndpointId(id.into()),
        action,
    }
}
fn root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "essim-rtu-resource-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
#[test]
fn exclusive_alias_failure_retry_apply_and_delete_release_real_port() {
    let root = root();
    let engine = WorkspaceEngine::with_tcp_runtime(root.clone()).unwrap();
    let (mut wire, slave) = serialport::TTYPort::pair().unwrap();
    let path = slave.name().unwrap();
    wire.set_timeout(std::time::Duration::from_secs(2)).unwrap();
    let alias = root.with_extension("serial-link");
    std::os::unix::fs::symlink(&path, &alias).unwrap();
    setup(&engine, "a", &path);
    setup(&engine, "b", alias.to_str().unwrap());
    engine
        .execute_slave(command("a", SlaveAction::Start { max_clients: 10 }))
        .unwrap();
    assert!(engine
        .execute_slave(command("b", SlaveAction::Start { max_clients: 10 }))
        .is_err());
    let mut packet = vec![1, 3, 0, 0, 0, 1];
    packet.extend(libmodbus_rs::rtu_crc(&packet).to_le_bytes());
    wire.write_all(&packet).unwrap();
    let mut response = [0; 7];
    wire.read_exact(&mut response).unwrap();
    assert_eq!(&response[..5], &[1, 3, 2, 0, 7]);
    // 运行中显式应用先释放串口，并保持停止。
    action(
        &engine,
        serde_json::json!({"kind":"apply","id":"a","resetIncompatible":false}),
    );
    engine
        .execute_slave(command("b", SlaveAction::Start { max_clients: 10 }))
        .unwrap();
    // 删除运行端点释放资源，不以目录移除代替停止。
    action(
        &engine,
        serde_json::json!({"kind":"deleteEndpoint","id":"b"}),
    );
    engine
        .execute_slave(command("a", SlaveAction::Start { max_clients: 10 }))
        .unwrap();
    assert_eq!(
        engine
            .execute_slave(command("a", SlaveAction::Stop))
            .unwrap()
            .state,
        SlaveState::Stopped
    );
    drop(engine);
    drop(slave);
    std::fs::remove_file(alias).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn missing_path_and_driver_loss_report_failure() {
    let root = root();
    let engine = WorkspaceEngine::with_tcp_runtime(root.clone()).unwrap();
    setup(&engine, "missing", "/dev/essim-missing-test-port");
    assert!(engine
        .execute_slave(command("missing", SlaveAction::Start { max_clients: 10 }))
        .is_err());
    let (wire, slave) = serialport::TTYPort::pair().unwrap();
    let path = slave.name().unwrap();
    setup(&engine, "live", &path);
    engine
        .execute_slave(command("live", SlaveAction::Start { max_clients: 10 }))
        .unwrap();
    drop(wire);
    let mut failed = false;
    for _ in 0..100 {
        let view = engine
            .query_slave(SlaveQuery {
                endpoint: EndpointId("live".into()),
                unit: None,
                points: vec![],
            })
            .unwrap();
        if view.state == SlaveState::Failed {
            failed = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(failed, "驱动断开必须报告失败，不能保持运行");
    drop(slave);
    drop(engine);
    std::fs::remove_dir_all(root).unwrap();
}
