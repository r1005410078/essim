//! 覆盖查询、快照与共享准入的基础设施契约。
use essim_core::{
    application::commands::{Action, Command},
    domain::identity::{EndpointId, UnitId},
};
use essim_infrastructure::workspace::WorkspaceEngine;
use serde_json::json;
fn configure(e: &WorkspaceEngine) {
    let model = json!({"id":"m","name":"M","points":[{"id":"p","name":"P","category":"measurement","area":"holding","address":0,"valueType":"u16","scale":"1","unit":"V","initial":{"kind":"unsigned","value":"12"},"initialEngineering":"12","byteOrder":"big","wordOrder":"big","note":"","slaveWritable":true,"masterPeriodMs":1000}]});
    let endpoint = json!({"id":"e","name":"E","role":"slave","transport":{"kind":"tcp","address":"127.0.0.1","port":1502},"units":[{"id":"u","name":"U","station":1,"enabled":true,"model":{"id":"m"},"device":null,"overrides":{"pointOverrides":{"p":{"initialEngineering":"20"}}}}]});
    e.execute(Command {
        command_id: "create".into(),
        action: serde_json::from_value(
            json!({"kind":"createEndpointWithModel","model":model,"endpoint":endpoint}),
        )
        .unwrap(),
    })
    .unwrap();
}
#[test]
fn effective_export_is_flattened_and_snapshot_is_independent() {
    let e = WorkspaceEngine::default();
    configure(&e);
    let effective = e
        .saved_effective_model(&EndpointId("e".into()), &UnitId("u".into()))
        .unwrap();
    assert_eq!(effective.points[0].initial_engineering, "20");
    let encoded = serde_json::to_string(&effective).unwrap();
    assert!(!encoded.contains("overrides"));
    let decoded: essim_core::domain::model::CollectionModel =
        serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, effective);
    e.execute(Command {
        command_id: "apply".into(),
        action: Action::Apply {
            id: EndpointId("e".into()),
            reset_incompatible: false,
        },
    })
    .unwrap();
    let snapshot = e.query().unwrap().catalog.values[&UnitId("u".into())].clone();
    let mut model = e.query().unwrap().catalog.models[0].clone();
    model.points[0].note = "changed".into();
    e.execute(Command {
        command_id: "save".into(),
        action: Action::SaveModel { model },
    })
    .unwrap();
    assert_eq!(snapshot.model.unwrap().points[0].note, "");
    assert_eq!(e.query_resources().unwrap().reserved.memory_bytes, 0);
}
#[cfg(feature = "tcp-runtime")]
#[test]
fn oversized_override_command_is_rejected_atomically_and_releases_budget() {
    use essim_infrastructure::{
        operation_gates::OperationGates,
        resources::{ResourceManager, ResourcePolicy},
    };
    let mut policy = ResourcePolicy::default();
    policy.limit.memory_bytes = 200_000;
    let resources = ResourceManager::with_policy(policy).unwrap();
    let root = std::env::temp_dir().join(format!("essim-overrides-budget-{}", std::process::id()));
    let e =
        WorkspaceEngine::with_runtime_resources(root.clone(), OperationGates::default(), resources)
            .unwrap();
    configure(&e);
    let before = e.query().unwrap().catalog;
    let mut endpoint = before.endpoints[0].clone();
    endpoint.units[0]
        .overrides
        .point_overrides
        .values_mut()
        .next()
        .unwrap()
        .note = Some("x".repeat(100_000));
    assert!(
        e.execute(Command {
            command_id: "over-budget".into(),
            action: Action::SaveEndpoint { endpoint }
        })
        .is_err()
    );
    assert_eq!(e.query().unwrap().catalog, before);
    assert_eq!(e.query_resources().unwrap().reserved.memory_bytes, 0);
    // 拒绝后正常复制、应用仍可用，不留下额度或孤立配置。
    e.execute(Command {
        command_id: "copy".into(),
        action: serde_json::from_value(json!({"kind":"copyEndpoint","source":"e","id":"copy"}))
            .unwrap(),
    })
    .unwrap();
    e.execute(Command {
        command_id: "apply".into(),
        action: Action::Apply {
            id: EndpointId("e".into()),
            reset_incompatible: false,
        },
    })
    .unwrap();
    assert_eq!(e.query_resources().unwrap().reserved.memory_bytes, 0);
    drop(e);
    let _ = std::fs::remove_dir_all(root);
}
#[test]
fn batch_apply_uses_effective_definitions_and_rechecks_override_content() {
    use essim_core::application::commands::batch_targets::{BatchScope, prepare_batch};
    use essim_core::domain::aggregates::batch::BatchOperation;
    let e = WorkspaceEngine::default();
    configure(&e);
    let id = EndpointId("e".into());
    let plan = prepare_batch(
        &e.query().unwrap().catalog,
        "batch".into(),
        BatchOperation::Apply,
        BatchScope::All {},
        1,
    )
    .unwrap();
    let gates = e.operation_gates();
    let permit = gates.acquire(&id).unwrap();
    e.apply_batch_target(&plan, &id, &permit).unwrap();
    assert_eq!(
        e.query().unwrap().catalog.values[&UnitId("u".into())]
            .model
            .as_ref()
            .unwrap()
            .points[0]
            .initial_engineering,
        "20"
    );
    let mut endpoint = e.query().unwrap().catalog.endpoints[0].clone();
    endpoint.units[0]
        .overrides
        .point_overrides
        .values_mut()
        .next()
        .unwrap()
        .initial_engineering = Some("30".into());
    e.execute(Command {
        command_id: "change".into(),
        action: Action::SaveEndpoint { endpoint },
    })
    .unwrap();
    assert!(e.apply_batch_target(&plan, &id, &permit).is_err());
    assert_eq!(e.query_resources().unwrap().reserved.memory_bytes, 0);
}
