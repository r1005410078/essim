//! 系统接口校验与会话预览消费，不修改系统网络。
use essim_core::application::commands::{Action, Command, partition::PartitionRequest};
use essim_infrastructure::workspace::WorkspaceEngine;
fn engine() -> WorkspaceEngine {
    let e = WorkspaceEngine::default();
    let model = serde_json::json!({"id":"m","name":"M","points":[{"id":"p","name":"p","category":"measurement","area":"holding","address":65535,"valueType":"u16","scale":"1","unit":"","initial":{"kind":"unsigned","value":"7"},"initialEngineering":"7","byteOrder":"big","wordOrder":"big","note":"","slaveWritable":true,"masterPeriodMs":1000}]});
    e.execute(Command {
        command_id: "save".into(),
        action: Action::SaveModel {
            model: serde_json::from_value(model).unwrap(),
        },
    })
    .unwrap();
    e
}
fn req(ip: &str) -> PartitionRequest {
    serde_json::from_value(serde_json::json!({"source":"m","batchId":"b","prefix":"组","groupSize":1000,"startIp":ip,"port":1080,"station":1,"folder":null})).unwrap()
}
#[test]
fn preview_does_not_create_and_commit_consumes_token_once() {
    let e = engine();
    let p = e.preview_partition(req("127.0.0.1")).unwrap();
    assert_eq!(p.preview.groups[0].ranges[0].1, 65535);
    assert!(e.query().unwrap().catalog.endpoints.is_empty());
    e.create_partition(&p.token).unwrap();
    assert_eq!(e.query().unwrap().catalog.endpoints.len(), 1);
    assert!(e.create_partition(&p.token).is_err());
}
#[test]
fn unconfigured_ip_rejected_without_partial_creation() {
    let e = engine();
    let preview = e.preview_partition(req("192.0.2.199")).unwrap();
    assert!(!preview.can_create);
    assert_eq!(preview.local_issues.len(), 1);
    assert_eq!(preview.preview.groups.len(), 1);
    assert!(e.create_partition(&preview.token).is_err());
    assert!(e.query().unwrap().catalog.endpoints.is_empty());
}

#[test]
fn discarded_preview_cannot_create_and_can_be_replaced() {
    let e = engine();
    let p = e.preview_partition(req("127.0.0.1")).unwrap();
    e.discard_partition(&p.token).unwrap();
    e.discard_partition(&p.token).unwrap();
    assert!(e.create_partition(&p.token).is_err());
    let next = e.preview_partition(req("127.0.0.1")).unwrap();
    e.create_partition(&next.token).unwrap();
}

#[test]
fn invalid_ips_keep_all_five_groups_visible_but_cannot_commit() {
    let e = engine();
    let mut model = e.query().unwrap().catalog.models[0].clone();
    let p = model.points[0].clone();
    model.points = (0..5000)
        .map(|i| {
            let mut p = p.clone();
            p.id = essim_core::domain::identity::PointId(format!("p{i}"));
            p.address = i * 2;
            p
        })
        .collect();
    e.execute(Command {
        command_id: "fill".into(),
        action: Action::SaveModel { model },
    })
    .unwrap();
    let p = e.preview_partition(req("192.0.2.199")).unwrap();
    assert_eq!(p.preview.total_points, 5000);
    assert_eq!(p.preview.groups.len(), 5);
    assert!(!p.can_create);
    assert!(!p.local_issues.is_empty());
    assert!(e.create_partition(&p.token).is_err());
    assert!(e.query().unwrap().catalog.endpoints.is_empty());
}

#[test]
fn configured_specific_and_wildcard_listener_conflicts_are_prechecked_without_binding() {
    for address in ["127.0.0.1","0.0.0.0"] {
        let e=engine();
        e.execute(Command {command_id:"endpoint".into(),action:Action::CreateEndpoint {endpoint:serde_json::from_value(serde_json::json!({"id":"existing","name":"existing","role":"slave","transport":{"kind":"tcp","address":address,"port":1080},"units":[{"id":"u","name":"U","station":1,"enabled":true,"model":{"id":"m"},"device":null}]})).unwrap()}}).unwrap();
        assert_eq!(e.preview_partition(req("127.0.0.1")).unwrap_err().code,"BINDING_CONFLICT");
        assert_eq!(e.query().unwrap().catalog.endpoints.len(),1);
    }
}

#[cfg(feature="tcp-runtime")]
#[test]
fn external_port_competition_after_preview_is_reported_only_at_actual_start() {
    use essim_core::application::commands::slave::{SlaveCommand,SlaveAction};
    use essim_core::domain::identity::EndpointId;
    let root=std::env::temp_dir().join(format!("essim-partition-race-{}",std::process::id()));
    let e=WorkspaceEngine::with_tcp_runtime(root.clone()).unwrap();
    let model=engine().query().unwrap().catalog.models[0].clone();
    e.execute(Command{command_id:"m".into(),action:Action::SaveModel {model}}).unwrap();
    let probe=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let port=probe.local_addr().unwrap().port();drop(probe);
    let mut r=req("127.0.0.1");r.port=port;
    let preview=e.preview_partition(r).unwrap();
    let external=std::net::TcpListener::bind(("127.0.0.1",port)).unwrap();
    e.create_partition(&preview.token).unwrap();
    let id=EndpointId("b:group:0:endpoint".into());
    e.execute(Command{command_id:"apply".into(),action:Action::Apply{id:id.clone(),reset_incompatible:false}}).unwrap();
    assert!(e.execute_slave(SlaveCommand{command_id:"start".into(),endpoint:id,action:SlaveAction::Start{max_clients:10}}).is_err());
    assert_eq!(e.query().unwrap().catalog.endpoints.len(),1);
    drop(external);e.shutdown_slave();let _=std::fs::remove_dir_all(root);
}
