//! 固定目标范围及逐项内容复核；不依赖点表选区，不复制实时值。
use essim_core::{
    application::{
        catalog::Catalog,
        commands::batch_targets::{BatchScope, prepare_batch},
    },
    domain::{
        aggregates::batch::BatchOperation,
        endpoint::EndpointConfig,
        identity::{EndpointId, FolderId},
        model::CollectionModel,
        navigation::Folder,
        values::UnitValues,
    },
};
fn endpoint(id: &str) -> EndpointConfig {
    serde_json::from_value(serde_json::json!({"id":id,"name":id,"role":"slave","transport":{"kind":"tcp","address":"127.0.0.1","port":1080},"units":[{"id":format!("{id}-u"),"name":"U","station":1,"enabled":true,"model":{"id":"m"},"device":null}]})).unwrap()
}
fn catalog() -> Catalog {
    let mut c = Catalog::default();
    c.endpoints = vec![endpoint("a"), endpoint("b"), endpoint("c")];
    let model: CollectionModel =
        serde_json::from_value(serde_json::json!({"id":"m","name":"M","points":[{"id":"p","name":"P","category":"measurement","area":"holding","address":0,"valueType":"u16","scale":"1","unit":"","initial":{"kind":"unsigned","value":"7"},"initialEngineering":"7","byteOrder":"big","wordOrder":"big","note":"","slaveWritable":true,"masterPeriodMs":1000}]})).unwrap();
    c.models.push(model.clone());
    c.applied = c.endpoints.clone();
    for e in &c.applied {
        c.values.insert(
            e.units[0].id.clone(),
            UnitValues {
                model: Some(model.clone().into()),
                ..Default::default()
            },
        );
    }
    for (id, parent) in [("root", None), ("child", Some("root")), ("other", None)] {
        c.navigation
            .save_folder(Folder {
                scope: Default::default(),
                icon: "folder".into(),
                color: "#477fbc".into(),
                id: FolderId(id.into()),
                name: id.into(),
                parent: parent.map(|p| FolderId(p.into())),
            })
            .unwrap();
    }
    c.navigation
        .members
        .insert(EndpointId("a".into()), FolderId("root".into()));
    c.navigation
        .members
        .insert(EndpointId("b".into()), FolderId("child".into()));
    c
}
fn selected(ids: &[&str]) -> BatchScope {
    BatchScope::Selected {
        ids: ids.iter().map(|v| EndpointId((*v).into())).collect(),
    }
}
#[test]
fn recursive_scope_deduplicates_and_freezes_membership() {
    let mut c = catalog();
    let plan = prepare_batch(
        &c,
        "b".into(),
        BatchOperation::Apply,
        BatchScope::Folder {
            id: FolderId("root".into()),
        },
        4,
    )
    .unwrap();
    assert_eq!(
        plan.targets(),
        vec![EndpointId("a".into()), EndpointId("b".into())]
    );
    c.navigation.members.remove(&EndpointId("a".into()));
    c.navigation
        .members
        .insert(EndpointId("c".into()), FolderId("child".into()));
    c.endpoints.push(endpoint("new"));
    assert_eq!(plan.targets().len(), 2);
    assert!(plan.revalidate(&c, &EndpointId("a".into())).is_ok());
    assert!(plan.revalidate(&c, &EndpointId("c".into())).is_err());
    let selected = prepare_batch(
        &c,
        "s".into(),
        BatchOperation::Stop,
        selected(&["b", "a", "b"]),
        4,
    )
    .unwrap();
    assert_eq!(
        selected.targets(),
        vec![EndpointId("b".into()), EndpointId("a".into())]
    );
}
#[test]
fn deleted_or_changed_target_does_not_invalidate_healthy_items() {
    let mut c = catalog();
    let p = prepare_batch(&c, "b".into(), BatchOperation::Apply, BatchScope::All {}, 4).unwrap();
    c.endpoints.retain(|e| e.id.0 != "a");
    c.endpoints[0].transport =
        serde_json::from_value(serde_json::json!({"kind":"tcp","address":"127.0.0.1","port":1081}))
            .unwrap();
    assert_eq!(
        p.revalidate(&c, &EndpointId("a".into())).unwrap_err().code,
        "BATCH_TARGET_DELETED"
    );
    assert_eq!(
        p.revalidate(&c, &EndpointId("b".into())).unwrap_err().code,
        "BATCH_TARGET_CHANGED"
    );
    assert!(p.revalidate(&c, &EndpointId("c".into())).is_ok());
}
#[test]
fn apply_checks_saved_models_start_checks_applied_definitions_and_values_are_ignored() {
    let mut c = catalog();
    let apply = prepare_batch(
        &c,
        "apply".into(),
        BatchOperation::Apply,
        selected(&["a"]),
        4,
    )
    .unwrap();
    let start = prepare_batch(
        &c,
        "start".into(),
        BatchOperation::Start,
        selected(&["a"]),
        4,
    )
    .unwrap();
    let stop = prepare_batch(&c, "stop".into(), BatchOperation::Stop, selected(&["a"]), 4).unwrap();
    c.models[0].name = "新源内容".into();
    assert!(apply.revalidate(&c, &EndpointId("a".into())).is_err());
    assert!(start.revalidate(&c, &EndpointId("a".into())).is_ok());
    let values = c.values.get_mut(&c.endpoints[0].units[0].id).unwrap();
    values.stale = true;
    values.raw.insert(
        essim_core::domain::identity::PointId("p".into()),
        essim_core::domain::point_table::RawValue::Unsigned(42),
    );
    assert!(start.revalidate(&c, &EndpointId("a".into())).is_ok());
    std::sync::Arc::make_mut(c.values
        .get_mut(&c.endpoints[0].units[0].id)
        .unwrap()
        .model
        .as_mut()
        .unwrap()).name = "新应用内容".into();
    assert!(start.revalidate(&c, &EndpointId("a".into())).is_err());
    assert!(stop.revalidate(&c, &EndpointId("a".into())).is_ok());
}
#[test]
fn model_snapshots_are_shared_and_preview_contains_no_model_content() {
    let c = catalog();
    let p = prepare_batch(&c, "b".into(), BatchOperation::Apply, BatchScope::All {}, 4).unwrap();
    assert_eq!(p.retained_model_count(), 1);
    let json = serde_json::to_value(p.preview()).unwrap();
    assert!(
        json.get("targets")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v.get("points").is_none() && v.get("models").is_none())
    );
    assert_eq!(p.targets().len(), 3);
}
#[test]
fn invalid_scope_empty_targets_and_cycles_fail_before_task_creation() {
    let mut c = catalog();
    assert!(prepare_batch(&c, "b".into(), BatchOperation::Stop, selected(&[]), 4).is_err());
    assert!(
        prepare_batch(
            &c,
            "b".into(),
            BatchOperation::Stop,
            selected(&["missing"]),
            4
        )
        .is_err()
    );
    assert!(
        prepare_batch(
            &c,
            "b".into(),
            BatchOperation::Stop,
            BatchScope::Folder {
                id: FolderId("missing".into())
            },
            4
        )
        .is_err()
    );
    c.navigation.folders[0].parent = Some(FolderId("child".into()));
    assert!(
        prepare_batch(
            &c,
            "b".into(),
            BatchOperation::Stop,
            BatchScope::Folder {
                id: FolderId("root".into())
            },
            4
        )
        .is_err()
    );
}

#[test]
fn default_concurrency_limits_and_distinct_applied_contents_are_explicit() {
    use essim_core::application::commands::batch_targets::BatchRequest;
    let mut c = catalog();
    let request: BatchRequest = serde_json::from_value(
        serde_json::json!({"id":"b","operation":"start","scope":{"kind":"all"}}),
    )
    .unwrap();
    assert_eq!(request.prepare(&c).unwrap().preview().concurrency, 4);
    assert!(
        serde_json::from_value::<BatchRequest>(
            serde_json::json!({"id":"b","operation":"start","scope":{"kind":"all","ids":["a"]}})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<BatchRequest>(serde_json::json!({"id":"b","operation":"start"}))
            .is_err()
    );
    for concurrency in [0, 9] {
        assert!(
            prepare_batch(
                &c,
                "b".into(),
                BatchOperation::Stop,
                BatchScope::All {},
                concurrency
            )
            .is_err()
        );
    }
    std::sync::Arc::make_mut(c.values
        .get_mut(&c.endpoints[0].units[0].id)
        .unwrap()
        .model
        .as_mut()
        .unwrap()).name = "独立已应用内容".into();
    let p = prepare_batch(&c, "b".into(), BatchOperation::Apply, BatchScope::All {}, 4).unwrap();
    assert_eq!(p.retained_model_count(), 2);
    c.endpoints = (0..1025).map(|i| endpoint(&format!("e{i}"))).collect();
    assert_eq!(
        prepare_batch(&c, "b".into(), BatchOperation::Stop, BatchScope::All {}, 4)
            .err()
            .unwrap()
            .code,
        "BATCH_TARGET_LIMIT"
    );
}

#[test]
fn applied_model_clones_share_definition_but_values_and_future_model_edits_are_independent() {
    let mut c=catalog();
    let first=c.values.values().next().unwrap().clone();
    let mut second=first.clone();
    assert!(std::sync::Arc::ptr_eq(first.model.as_ref().unwrap(),second.model.as_ref().unwrap()));
    second.raw.insert(essim_core::domain::identity::PointId("private-value".into()),essim_core::domain::point_table::RawValue::Unsigned(42));
    assert_ne!(first.raw,second.raw);
    std::sync::Arc::make_mut(second.model.as_mut().unwrap()).name="独立改动".into();
    assert_ne!(first.model.as_ref().unwrap().name,second.model.as_ref().unwrap().name);
    c.models[0].name="后续保存内容".into();
    assert_ne!(first.model.as_ref().unwrap().name,c.models[0].name);
}
