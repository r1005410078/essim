//! 模型到 Unit 单层覆盖的业务契约，不依赖协议执行器。
use essim_core::{application::workspace::*, domain::foundation::*};
use serde_json::{Value, json};
#[derive(Default)]
struct Memory(Catalog);
impl ConfigRepository for Memory {
    fn load(&self) -> Catalog {
        self.0.clone()
    }
    fn commit(&mut self, c: Catalog) -> Result<(), Error> {
        self.0 = c;
        Ok(())
    }
}
fn model() -> Value {
    json!({"id":"m","name":"M","points":[{"id":"p","name":"P","category":"measurement","area":"holding","address":0,"valueType":"u16","scale":"1","unit":"V","initial":{"kind":"unsigned","value":"12"},"initialEngineering":"12","byteOrder":"big","wordOrder":"big","note":"","slaveWritable":false,"masterPeriodMs":1000}]})
}
fn endpoint() -> Value {
    json!({"id":"e","name":"E","role":"slave","transport":{"kind":"tcp","address":"127.0.0.1","port":1502},"units":[{"id":"u","name":"A","station":1,"enabled":true,"model":{"id":"m"},"device":null,"overrides":{"pointOverrides":{"p":{"address":10,"initialEngineering":"20"}}}}]})
}
fn workspace() -> Workspace<Memory> {
    let mut c = Catalog::default();
    c.models.push(serde_json::from_value(model()).unwrap());
    Workspace::new(Memory(c))
}
fn run(w: &mut Workspace<Memory>, action: Value) -> Result<Receipt, Error> {
    w.execute(Command {
        command_id: "test".into(),
        action: serde_json::from_value(action).unwrap(),
    })
    .map(|r| r.0)
}
fn apply(w: &mut Workspace<Memory>) {
    run(
        w,
        json!({"kind":"apply","id":"e","resetIncompatible":false}),
    )
    .unwrap();
}
#[test]
fn inline_creation_is_atomic_and_does_not_reuse_a_name() {
    let mut w = workspace();
    let before = w.query().catalog;
    let mut e = endpoint();
    e["units"][0]["station"] = json!(0);
    e["units"][0]["model"]["id"] = json!("new");
    e["units"][0]["overrides"] = json!({});
    let m = json!({"id":"new","name":"M","points":[]});
    assert!(
        run(
            &mut w,
            json!({"kind":"createEndpointWithModel","endpoint":e,"model":m})
        )
        .is_err()
    );
    assert_eq!(w.query().catalog, before);
    e["units"][0]["station"] = json!(1);
    run(
        &mut w,
        json!({"kind":"createEndpointWithModel","endpoint":e,"model":m}),
    )
    .unwrap();
    assert_eq!(w.query().catalog.models.len(), 2);
    assert!(w.query().catalog.applied.is_empty());
}
#[test]
fn shared_model_different_effective_points_and_reset_snapshot() {
    let mut w = workspace();
    let e = endpoint();
    run(&mut w, json!({"kind":"createEndpoint","endpoint":e})).unwrap();
    let mut e = e;
    let mut other = e["units"][0].clone();
    other["id"] = json!("v");
    other["station"] = json!(2);
    other["overrides"] = json!({});
    e["units"].as_array_mut().unwrap().push(other);
    run(&mut w, json!({"kind":"saveEndpoint","endpoint":e})).unwrap();
    apply(&mut w);
    let c = w.query().catalog;
    assert_eq!(
        c.values[&UnitId("u".into())].model.as_ref().unwrap().points[0].address,
        10
    );
    assert_eq!(
        c.values[&UnitId("v".into())].model.as_ref().unwrap().points[0].address,
        0
    );
    assert_eq!(
        c.values[&UnitId("u".into())].raw[&PointId("p".into())],
        RawValue::Unsigned(20)
    );
    e["units"][0]["overrides"]["pointOverrides"]["p"]["initialEngineering"] = json!("30");
    run(&mut w, json!({"kind":"saveEndpoint","endpoint":e})).unwrap();
    run(&mut w, json!({"kind":"resetUnit","id":"e","unit":"u"})).unwrap();
    assert_eq!(
        w.query().catalog.values[&UnitId("u".into())].raw[&PointId("p".into())],
        RawValue::Unsigned(20)
    );
    apply(&mut w);
    assert_eq!(
        w.query().catalog.values[&UnitId("u".into())].raw[&PointId("p".into())],
        RawValue::Unsigned(20)
    );
    run(&mut w, json!({"kind":"resetUnit","id":"e","unit":"u"})).unwrap();
    assert_eq!(
        w.query().catalog.values[&UnitId("u".into())].raw[&PointId("p".into())],
        RawValue::Unsigned(30)
    );
}
#[test]
fn invalid_override_and_base_deletion_keep_authority() {
    let mut w = workspace();
    let mut e = endpoint();
    e["units"][0]["overrides"]["pointOverrides"]["p"]["scale"] = json!("3");
    assert!(run(&mut w, json!({"kind":"createEndpoint","endpoint":e})).is_err());
    assert!(w.query().catalog.endpoints.is_empty());
    run(
        &mut w,
        json!({"kind":"createEndpoint","endpoint":endpoint()}),
    )
    .unwrap();
    let before = w.query().catalog;
    assert!(
        run(
            &mut w,
            json!({"kind":"saveModel","model":{"id":"m","name":"M","points":[]}})
        )
        .is_err()
    );
    assert_eq!(w.query().catalog, before);
}
#[test]
fn local_point_conflict_checks_disabled_unit() {
    let mut w = workspace();
    let mut e = endpoint();
    let mut p = model()["points"][0].clone();
    p["id"] = json!("local");
    p["address"] = json!(20);
    e["units"][0]["enabled"] = json!(false);
    e["units"][0]["overrides"]["addedPoints"] = json!([p]);
    run(&mut w, json!({"kind":"createEndpoint","endpoint":e})).unwrap();
    let mut m = model();
    m["points"][0]["id"] = json!("other");
    m["points"][0]["address"] = json!(20);
    m["points"]
        .as_array_mut()
        .unwrap()
        .insert(0, model()["points"][0].clone());
    let before = w.query().catalog;
    assert!(run(&mut w, json!({"kind":"saveModel","model":m})).is_err());
    assert_eq!(w.query().catalog, before);
}
#[test]
fn old_units_deserialize_as_empty_overrides() {
    let mut e = endpoint();
    e["units"][0].as_object_mut().unwrap().remove("overrides");
    let mut w = workspace();
    run(&mut w, json!({"kind":"createEndpoint","endpoint":e})).unwrap();
    apply(&mut w);
    assert_eq!(
        w.query().catalog.values[&UnitId("u".into())].raw[&PointId("p".into())],
        RawValue::Unsigned(12)
    );
}
#[test]
fn promotion_is_selective_atomic_and_rejects_changed_previews() {
    use essim_core::application::commands::overrides::{OverrideSelection, preview_promotion};
    let mut w = workspace();
    let mut e = endpoint();
    e["units"][0]["overrides"]["pointOverrides"]["p"]["note"] = json!("A note");
    run(&mut w, json!({"kind":"createEndpoint","endpoint":e})).unwrap();
    let selection: OverrideSelection =
        serde_json::from_value(json!({"fields":{"p":["note"]}})).unwrap();
    let plan = preview_promotion(
        w.catalog_ref(),
        EndpointId("e".into()),
        UnitId("u".into()),
        selection.clone(),
    )
    .unwrap();
    let mut m = model();
    m["name"] = json!("renamed");
    run(&mut w, json!({"kind":"saveModel","model":m})).unwrap();
    let before = w.query().catalog;
    assert_eq!(
        run(&mut w, json!({"kind":"promoteOverrides","plan":plan}))
            .unwrap_err()
            .code,
        "PREVIEW_CHANGED"
    );
    assert_eq!(w.query().catalog, before);
    let plan = preview_promotion(
        w.catalog_ref(),
        EndpointId("e".into()),
        UnitId("u".into()),
        selection,
    )
    .unwrap();
    run(&mut w, json!({"kind":"promoteOverrides","plan":plan})).unwrap();
    let c = w.query().catalog;
    assert_eq!(c.models[0].points[0].note, "A note");
    assert!(
        c.endpoints[0].units[0].overrides.point_overrides[&PointId("p".into())]
            .note
            .is_none()
    );
    assert_eq!(
        c.endpoints[0].units[0].overrides.point_overrides[&PointId("p".into())].address,
        Some(10)
    );
    assert!(c.applied.is_empty());
}
#[test]
fn promotion_adds_local_identity_and_extraction_generates_independent_identity() {
    use essim_core::application::commands::overrides::preview_promotion;
    let mut w = workspace();
    let mut e = endpoint();
    let mut p = model()["points"][0].clone();
    p["id"] = json!("local");
    p["address"] = json!(22);
    e["units"][0]["overrides"]["addedPoints"] = json!([p]);
    run(&mut w, json!({"kind":"createEndpoint","endpoint":e})).unwrap();
    let plan = preview_promotion(
        w.catalog_ref(),
        EndpointId("e".into()),
        UnitId("u".into()),
        serde_json::from_value(json!({"added":["local"]})).unwrap(),
    )
    .unwrap();
    run(&mut w, json!({"kind":"promoteOverrides","plan":plan})).unwrap();
    assert_eq!(
        w.query().catalog.models[0].points[1].id,
        PointId("local".into())
    );
    assert!(
        w.query().catalog.endpoints[0].units[0]
            .overrides
            .added_points
            .is_empty()
    );
    run(&mut w,json!({"kind":"extractUnitModel","endpoint":"e","unit":"u","id":"new","name":"Independent"})).unwrap();
    let c = w.query().catalog;
    assert_eq!(c.models[1].points[0].address, 10);
    assert_ne!(c.models[1].points[0].id, PointId("p".into()));
    assert_eq!(c.endpoints[0].units[0].model.id, ModelId("new".into()));
    assert!(c.endpoints[0].units[0].overrides.is_empty());
}
#[test]
fn hiding_is_local_and_deleted_hidden_marker_is_cleaned() {
    let mut w = workspace();
    let mut e = endpoint();
    e["units"][0]["overrides"] = json!({"removedPointIds":["p"]});
    run(&mut w, json!({"kind":"createEndpoint","endpoint":e})).unwrap();
    assert_eq!(w.query().catalog.models[0].points.len(), 1);
    run(
        &mut w,
        json!({"kind":"saveModel","model":{"id":"m","name":"M","points":[]}}),
    )
    .unwrap();
    assert!(w.query().catalog.endpoints[0].units[0].overrides.is_empty());
}
#[test]
fn primary_type_change_cannot_reinterpret_an_override_initial() {
    let mut w = workspace();
    run(
        &mut w,
        json!({"kind":"createEndpoint","endpoint":endpoint()}),
    )
    .unwrap();
    let mut e = serde_json::to_value(&w.query().catalog.endpoints[0]).unwrap();
    e["units"][0]["overrides"]["pointOverrides"]["p"]["initialEngineering"] = json!("60000");
    run(&mut w, json!({"kind":"saveEndpoint","endpoint":e})).unwrap();
    let before = w.query().catalog;
    let mut m = model();
    m["points"][0]["valueType"] = json!("i16");
    m["points"][0]["initial"] = json!({"kind":"signed","value":"12"});
    assert!(run(&mut w, json!({"kind":"saveModel","model":m})).is_err());
    assert_eq!(w.query().catalog, before);
}
#[test]
fn copying_has_new_local_point_ids_and_changing_model_requires_clear_overrides() {
    let mut w = workspace();
    let mut e = endpoint();
    let mut p = model()["points"][0].clone();
    p["id"] = json!("local");
    p["address"] = json!(22);
    e["units"][0]["overrides"]["addedPoints"] = json!([p]);
    run(&mut w, json!({"kind":"createEndpoint","endpoint":e})).unwrap();
    run(
        &mut w,
        json!({"kind":"copyEndpoint","source":"e","id":"copy"}),
    )
    .unwrap();
    let c = w.query().catalog;
    assert_ne!(
        c.endpoints[1].units[0].overrides.added_points[0].id,
        c.endpoints[0].units[0].overrides.added_points[0].id
    );
    let mut m = model();
    m["id"] = json!("other");
    run(&mut w, json!({"kind":"saveModel","model":m})).unwrap();
    e["units"][0]["model"]["id"] = json!("other");
    assert_eq!(
        run(&mut w, json!({"kind":"saveEndpoint","endpoint":e}))
            .unwrap_err()
            .code,
        "OVERRIDES_REQUIRE_DISPOSITION"
    );
}

#[test]
fn promotion_requires_dependent_fields_and_keeps_other_explicit_overrides() {
    use essim_core::application::commands::overrides::preview_promotion;
    let mut w = workspace();
    let mut e = endpoint();
    e["units"][0]["overrides"]["pointOverrides"]["p"] =
        json!({"valueType":"u32","initialEngineering":"70000"});
    run(&mut w, json!({"kind":"createEndpoint","endpoint":e})).unwrap();
    let mut other = e["units"][0].clone();
    other["id"] = json!("v");
    other["station"] = json!(2);
    other["overrides"] = json!({"pointOverrides":{"p":{"initialEngineering":"25"}}});
    e["units"].as_array_mut().unwrap().push(other);
    run(&mut w, json!({"kind":"saveEndpoint","endpoint":e})).unwrap();
    let before = w.query().catalog;
    let preview = |w: &Workspace<Memory>, selection: Value| {
        preview_promotion(
            w.catalog_ref(),
            EndpointId("e".into()),
            UnitId("u".into()),
            serde_json::from_value(selection).unwrap(),
        )
    };
    assert!(preview(&w, json!({"fields":{"p":["initialEngineering"]}})).is_err());
    assert_eq!(preview(&w, json!({})).unwrap_err().code, "EMPTY_SELECTION");
    assert_eq!(w.query().catalog, before);
    let plan = preview(
        &w,
        json!({"fields":{"p":["valueType","initialEngineering"]}}),
    )
    .unwrap();
    run(&mut w, json!({"kind":"promoteOverrides","plan":plan})).unwrap();
    apply(&mut w);
    let c = w.query().catalog;
    assert!(c.endpoints[0].units[0].overrides.is_empty());
    assert_eq!(
        c.values[&UnitId("u".into())].raw[&PointId("p".into())],
        RawValue::Unsigned(70000)
    );
    assert_eq!(
        c.values[&UnitId("v".into())].raw[&PointId("p".into())],
        RawValue::Unsigned(25)
    );
}
#[test]
fn promotion_rejects_shared_address_conflict_and_source_race() {
    use essim_core::application::commands::overrides::preview_promotion;
    let mut w = workspace();
    let mut e = endpoint();
    run(&mut w, json!({"kind":"createEndpoint","endpoint":e})).unwrap();
    let mut other = e["units"][0].clone();
    other["id"] = json!("v");
    other["station"] = json!(2);
    let mut local = model()["points"][0].clone();
    local["id"] = json!("local");
    local["address"] = json!(10);
    other["overrides"] = json!({"addedPoints":[local]});
    e["units"].as_array_mut().unwrap().push(other);
    run(&mut w, json!({"kind":"saveEndpoint","endpoint":e})).unwrap();
    let before = w.query().catalog;
    let preview = |w: &Workspace<Memory>, field: &str| {
        preview_promotion(
            w.catalog_ref(),
            EndpointId("e".into()),
            UnitId("u".into()),
            serde_json::from_value(json!({"fields":{"p":[field]}})).unwrap(),
        )
    };
    assert!(preview(&w, "address").is_err());
    assert_eq!(w.query().catalog, before);
    let plan = preview(&w, "initialEngineering").unwrap();
    e["units"][0]["overrides"]["pointOverrides"]["p"]["initialEngineering"] = json!("30");
    run(&mut w, json!({"kind":"saveEndpoint","endpoint":e})).unwrap();
    let before = w.query().catalog;
    assert_eq!(
        run(&mut w, json!({"kind":"promoteOverrides","plan":plan}))
            .unwrap_err()
            .code,
        "PREVIEW_CHANGED"
    );
    assert_eq!(w.query().catalog, before);
}
#[test]
fn equal_effective_snapshots_share_definitions_but_not_values() {
    let mut w = workspace();
    let mut e = endpoint();
    run(&mut w, json!({"kind":"createEndpoint","endpoint":e})).unwrap();
    let mut other = e["units"][0].clone();
    other["id"] = json!("v");
    other["station"] = json!(2);
    e["units"].as_array_mut().unwrap().push(other);
    run(&mut w, json!({"kind":"saveEndpoint","endpoint":e})).unwrap();
    apply(&mut w);
    let c = w.query().catalog;
    assert!(std::sync::Arc::ptr_eq(
        c.values[&UnitId("u".into())].model.as_ref().unwrap(),
        c.values[&UnitId("v".into())].model.as_ref().unwrap()
    ));
    let mut changed = c.models[0].clone();
    changed.points[0].note = "new base".into();
    run(
        &mut w,
        serde_json::json!({"kind":"saveModel","model":changed}),
    )
    .unwrap();
    assert_eq!(
        c.values[&UnitId("u".into())].model.as_ref().unwrap().points[0].note,
        ""
    );
}
#[test]
fn repository_failure_never_partially_promotes_or_extracts() {
    use essim_core::application::commands::overrides::preview_promotion;
    struct Reject(Catalog);
    impl ConfigRepository for Reject {
        fn load(&self) -> Catalog {
            self.0.clone()
        }
        fn commit(&mut self, _: Catalog) -> Result<(), Error> {
            Err(Error::new("TEST_COMMIT_FAILED", "注入提交失败"))
        }
    }
    let mut source = workspace();
    run(
        &mut source,
        json!({"kind":"createEndpoint","endpoint":endpoint()}),
    )
    .unwrap();
    let before = source.query().catalog;
    let plan = preview_promotion(
        &before,
        EndpointId("e".into()),
        UnitId("u".into()),
        serde_json::from_value(json!({"fields":{"p":["initialEngineering"]}})).unwrap(),
    )
    .unwrap();
    let mut w = Workspace::new(Reject(before.clone()));
    for action in [
        json!({"kind":"promoteOverrides","plan":plan}),
        json!({"kind":"extractUnitModel","endpoint":"e","unit":"u","id":"new","name":"New"}),
    ] {
        let result = w.execute(Command {
            command_id: "fail".into(),
            action: serde_json::from_value(action).unwrap(),
        });
        assert_eq!(result.unwrap_err().code, "TEST_COMMIT_FAILED");
        assert_eq!(w.query().catalog, before);
    }
}
#[test]
fn promoted_local_points_keep_saved_order_not_identity_sort_order() {
    use essim_core::application::commands::overrides::preview_promotion;
    let mut w = workspace();
    let mut e = endpoint();
    let points: Vec<_> = [("z", 20), ("a", 21)]
        .into_iter()
        .map(|(id, address)| {
            let mut p = model()["points"][0].clone();
            p["id"] = json!(id);
            p["address"] = json!(address);
            p
        })
        .collect();
    e["units"][0]["overrides"]["addedPoints"] = json!(points);
    run(&mut w, json!({"kind":"createEndpoint","endpoint":e})).unwrap();
    let plan = preview_promotion(
        w.catalog_ref(),
        EndpointId("e".into()),
        UnitId("u".into()),
        serde_json::from_value(json!({"added":["z","a"]})).unwrap(),
    )
    .unwrap();
    assert_eq!(
        plan.after
            .points
            .iter()
            .map(|p| p.id.0.as_str())
            .collect::<Vec<_>>(),
        vec!["p", "z", "a"]
    );
}
