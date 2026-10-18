//! M03配置与受控停机测试，不包含真实协议资源。
use essim_core::{application::workspace::*, domain::foundation::*};
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
fn endpoint() -> EndpointConfig {
    serde_json::from_value(serde_json::json!({"id":"e","name":"E","role":"slave","transport":{"kind":"tcp","address":"127.0.0.1","port":1502},"units":[{"id":"u","name":"Unit 1","station":1,"enabled":true,"model":{"id":"m"},"device":null}]})).unwrap()
}
fn workspace() -> Workspace<Memory> {
    let mut c = Catalog::default();
    c.models.push(CollectionModel {
        id: ModelId("m".into()),
        name: "M".into(),
        points: vec![],
    });
    Workspace::new(Memory(c))
}
fn run(w: &mut Workspace<Memory>, a: serde_json::Value) -> Result<Receipt, Error> {
    w.execute(Command {
        command_id: "test".into(),
        action: serde_json::from_value(a).unwrap(),
    })
    .map(|v| v.0)
}
#[test]
fn create_is_atomic_and_save_cannot_resurrect_or_change_type() {
    let mut w = workspace();
    let e = endpoint();
    assert_eq!(
        run(
            &mut w,
            serde_json::json!({"kind":"saveEndpoint","endpoint":e})
        )
        .unwrap_err()
        .code,
        "NOT_FOUND"
    );
    let mut invalid = e.clone();
    invalid.units.clear();
    assert!(run(
        &mut w,
        serde_json::json!({"kind":"createEndpoint","endpoint":invalid})
    )
    .is_err());
    assert!(w.query().catalog.endpoints.is_empty());
    run(
        &mut w,
        serde_json::json!({"kind":"createEndpoint","endpoint":e}),
    )
    .unwrap();
    assert!(run(
        &mut w,
        serde_json::json!({"kind":"createEndpoint","endpoint":e})
    )
    .is_err());
    let mut changed = e.clone();
    changed.role = Role::Master;
    assert_eq!(
        run(
            &mut w,
            serde_json::json!({"kind":"saveEndpoint","endpoint":changed})
        )
        .unwrap_err()
        .code,
        "ENDPOINT_TYPE_IMMUTABLE"
    );
    assert_eq!(w.query().catalog.endpoints[0], e);
}
#[test]
fn duplicate_station_locates_both_units_and_keeps_authority() {
    let mut w = workspace();
    let e = endpoint();
    run(
        &mut w,
        serde_json::json!({"kind":"createEndpoint","endpoint":e}),
    )
    .unwrap();
    let mut bad = e.clone();
    let mut u = bad.units[0].clone();
    u.id = UnitId("other".into());
    bad.units.push(u);
    let err = run(
        &mut w,
        serde_json::json!({"kind":"saveEndpoint","endpoint":bad}),
    )
    .unwrap_err();
    let err = serde_json::to_value(err).unwrap();
    assert_eq!(err["endpointId"], "e");
    assert_eq!(err["unitId"], "other");
    assert_eq!(err["relatedUnitId"], "u");
    assert_eq!(err["field"], "station");
    assert_eq!(w.query().catalog.endpoints[0], e);
}
#[test]
fn copy_has_fresh_units_same_references_and_no_application() {
    let mut w = workspace();
    let e = endpoint();
    run(
        &mut w,
        serde_json::json!({"kind":"createEndpoint","endpoint":e}),
    )
    .unwrap();
    run(
        &mut w,
        serde_json::json!({"kind":"apply","id":"e","resetIncompatible":false}),
    )
    .unwrap();
    run(
        &mut w,
        serde_json::json!({"kind":"copyEndpoint","source":"e","id":"copy"}),
    )
    .unwrap();
    let c = w.query().catalog;
    assert_eq!(c.endpoints[1].name, "E 副本");
    assert_ne!(c.endpoints[1].units[0].id, e.units[0].id);
    assert_eq!(c.endpoints[1].units[0].model, e.units[0].model);
    assert_eq!(c.endpoints[1].transport, e.transport);
    assert_eq!(c.applied.len(), 1);
    assert_eq!(c.values.len(), 1);
    run(
        &mut w,
        serde_json::json!({"kind":"deleteEndpoint","id":"e"}),
    )
    .unwrap();
    let c = w.query().catalog;
    assert_eq!(c.endpoints.len(), 1);
    assert!(c.values.is_empty());
    assert!(c.applied.is_empty());
    assert_eq!(c.models.len(), 1);
    assert!(run(
        &mut w,
        serde_json::json!({"kind":"saveEndpoint","endpoint":e})
    )
    .is_err());
}
#[test]
fn metadata_is_not_pending_but_protocol_change_is() {
    let mut w = workspace();
    let e = endpoint();
    run(
        &mut w,
        serde_json::json!({"kind":"createEndpoint","endpoint":e}),
    )
    .unwrap();
    run(
        &mut w,
        serde_json::json!({"kind":"apply","id":"e","resetIncompatible":false}),
    )
    .unwrap();
    let mut e = e;
    e.name = "rename".into();
    e.units[0].name = "rename unit".into();
    run(
        &mut w,
        serde_json::json!({"kind":"saveEndpoint","endpoint":e}),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(w.query()).unwrap()["applicationMatches"]["e"]["u"],
        true
    );
    e.units[0].station = 2;
    run(
        &mut w,
        serde_json::json!({"kind":"saveEndpoint","endpoint":e}),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(w.query()).unwrap()["applicationMatches"]["e"]["u"],
        false
    );
}
#[test]
fn folders_reject_cycles_nonempty_delete_and_missing_targets() {
    let mut w = workspace();
    run(
        &mut w,
        serde_json::json!({"kind":"saveFolder","folder":{"id":"a","name":"A","parent":null}}),
    )
    .unwrap();
    run(
        &mut w,
        serde_json::json!({"kind":"saveFolder","folder":{"id":"b","name":"B","parent":"a"}}),
    )
    .unwrap();
    let before = w.query().catalog;
    assert!(run(
        &mut w,
        serde_json::json!({"kind":"saveFolder","folder":{"id":"a","name":"A","parent":"b"}})
    )
    .is_err());
    assert_eq!(w.query().catalog, before);
    assert!(run(&mut w, serde_json::json!({"kind":"deleteFolder","id":"a"})).is_err());
    run(
        &mut w,
        serde_json::json!({"kind":"createEndpoint","endpoint":endpoint()}),
    )
    .unwrap();
    run(
        &mut w,
        serde_json::json!({"kind":"moveEndpoint","id":"e","parent":"b"}),
    )
    .unwrap();
    assert!(run(&mut w, serde_json::json!({"kind":"deleteFolder","id":"b"})).is_err());
    run(
        &mut w,
        serde_json::json!({"kind":"deleteEndpoint","id":"e"}),
    )
    .unwrap();
    run(&mut w, serde_json::json!({"kind":"deleteFolder","id":"b"})).unwrap();
    run(&mut w, serde_json::json!({"kind":"deleteFolder","id":"a"})).unwrap();
}
#[test]
fn disabled_values_and_removal_follow_explicit_application_and_reset_snapshot() {
    let mut c = Catalog::default();
    let m:CollectionModel=serde_json::from_value(serde_json::json!({"id":"m","name":"M","points":[{"id":"p","name":"P","category":"measurement","area":"holding","address":0,"valueType":"u16","scale":"1","unit":"V","initial":{"kind":"unsigned","value":"12"},"initialEngineering":"12","byteOrder":"big","wordOrder":"big","note":"","slaveWritable":false,"masterPeriodMs":1000}]})).unwrap();
    c.models.push(m.clone());
    let mut e = endpoint();
    let mut other = e.units[0].clone();
    other.id = UnitId("v".into());
    other.station = 2;
    e.units.push(other);
    c.endpoints.push(e.clone());
    c.applied.push(e.clone());
    for u in &e.units {
        let mut values = reset_values(Role::Slave, &m);
        values
            .raw
            .insert(PointId("p".into()), RawValue::Unsigned(99));
        c.values.insert(u.id.clone(), values);
    }
    let mut w = Workspace::new(Memory(c));
    for enabled in [false, true] {
        e.units[0].enabled = enabled;
        run(
            &mut w,
            serde_json::json!({"kind":"saveEndpoint","endpoint":e}),
        )
        .unwrap();
        run(
            &mut w,
            serde_json::json!({"kind":"apply","id":"e","resetIncompatible":false}),
        )
        .unwrap();
        assert_eq!(
            w.query().catalog.values[&UnitId("u".into())].raw[&PointId("p".into())],
            RawValue::Unsigned(99)
        );
    }
    let mut changed = m;
    changed.points[0].initial = RawValue::Unsigned(23);
    changed.points[0].initial_engineering = "23".into();
    run(
        &mut w,
        serde_json::json!({"kind":"saveModel","model":changed}),
    )
    .unwrap();
    run(
        &mut w,
        serde_json::json!({"kind":"resetUnit","id":"e","unit":"u"}),
    )
    .unwrap();
    let c = w.query().catalog;
    assert_eq!(
        c.values[&UnitId("u".into())].raw[&PointId("p".into())],
        RawValue::Unsigned(12)
    );
    assert_eq!(
        c.values[&UnitId("v".into())].raw[&PointId("p".into())],
        RawValue::Unsigned(99)
    );
    e.units.clear();
    run(
        &mut w,
        serde_json::json!({"kind":"saveEndpoint","endpoint":e}),
    )
    .unwrap();
    assert_eq!(w.query().catalog.values.len(), 2);
    assert_eq!(
        run(&mut w, serde_json::json!({"kind":"start","id":"e"}))
            .unwrap_err()
            .code,
        "NO_ENABLED_UNIT"
    );
    run(
        &mut w,
        serde_json::json!({"kind":"apply","id":"e","resetIncompatible":false}),
    )
    .unwrap();
    assert!(w.query().catalog.values.is_empty());
}
#[test]
fn create_missing_reference_is_located_and_copy_failure_leaves_no_partial_object() {
    let mut w = workspace();
    let mut e = endpoint();
    e.units[0].model.id = ModelId("deleted".into());
    let error = run(
        &mut w,
        serde_json::json!({"kind":"createEndpoint","endpoint":e}),
    )
    .unwrap_err();
    assert_eq!(error.unit_id.as_deref(), Some("u"));
    assert!(w.query().catalog.endpoints.is_empty());
    assert!(run(
        &mut w,
        serde_json::json!({"kind":"copyEndpoint","source":"deleted","id":"copy"})
    )
    .is_err());
    assert!(w.query().catalog.endpoints.is_empty());
}
#[test]
fn commit_failure_after_release_preserves_application_and_real_stop() {
    struct Failing(Catalog);
    impl ConfigRepository for Failing {
        fn load(&self) -> Catalog {
            self.0.clone()
        }
        fn commit(&mut self, _: Catalog) -> Result<(), Error> {
            Err(Error::new("STORE_FAILED", "写入失败"))
        }
    }
    let e = endpoint();
    let mut c = Catalog::default();
    c.models.push(CollectionModel {
        id: ModelId("m".into()),
        name: "M".into(),
        points: vec![],
    });
    c.endpoints.push(e.clone());
    c.applied.push(e.clone());
    c.values
        .insert(e.units[0].id.clone(), UnitValues::default());
    let mut w = Workspace::new(Failing(c.clone()));
    for kind in ["deleteEndpoint", "apply"] {
        let action = serde_json::from_value(
            serde_json::json!({"kind":kind,"id":"e","resetIncompatible":false}),
        )
        .unwrap();
        assert_eq!(
            w.execute(Command {
                command_id: "test".into(),
                action
            })
            .unwrap_err()
            .code,
            "STORE_FAILED"
        );
        assert_eq!(w.query().catalog, c);
        assert_eq!(w.application_state(&e.id), Some(ApplyState::Stopped));
    }
}
#[test]
fn transport_kind_is_immutable_and_same_stopped_resource_is_allowed() {
    let mut w = workspace();
    let mut e = endpoint();
    run(
        &mut w,
        serde_json::json!({"kind":"createEndpoint","endpoint":e}),
    )
    .unwrap();
    e.transport = Transport::Rtu {
        path: "/dev/test".into(),
        baud: 19200,
        parity: Parity::None,
        data_bits: 8,
        stop_bits: 1,
    };
    assert_eq!(
        run(
            &mut w,
            serde_json::json!({"kind":"saveEndpoint","endpoint":e})
        )
        .unwrap_err()
        .code,
        "ENDPOINT_TYPE_IMMUTABLE"
    );
    run(
        &mut w,
        serde_json::json!({"kind":"copyEndpoint","source":"e","id":"copy"}),
    )
    .unwrap();
    assert_eq!(w.query().catalog.endpoints.len(), 2);
}
#[test]
fn station_allocation_bounds_exhaustion_and_duplicate_identity() {
    let mut e = endpoint();
    assert_eq!(e.next_station(), Some(2));
    for station in 2..=247 {
        let mut u = e.units[0].clone();
        u.id = UnitId(format!("u{station}"));
        u.station = station;
        e.units.push(u);
    }
    assert_eq!(e.next_station(), None);
    e.units.remove(1);
    assert_eq!(e.next_station(), Some(2));
    e.validate().unwrap();
    e.units[1].id = e.units[0].id.clone();
    assert_eq!(e.validate().unwrap_err().field.as_deref(), Some("id"));
    for station in [0, 248] {
        let mut e = endpoint();
        e.units[0].station = station;
        assert_eq!(e.validate().unwrap_err().field.as_deref(), Some("station"));
    }
}
#[test]
fn copying_invalid_source_reference_and_reset_master_cache_are_atomic() {
    let mut c = Catalog::default();
    let mut e = endpoint();
    c.endpoints.push(e.clone());
    let mut w = Workspace::new(Memory(c.clone()));
    assert_eq!(
        run(
            &mut w,
            serde_json::json!({"kind":"copyEndpoint","source":"e","id":"copy"})
        )
        .unwrap_err()
        .code,
        "MODEL_NOT_FOUND"
    );
    assert_eq!(w.query().catalog, c);
    e.role = Role::Master;
    let m = CollectionModel {
        id: ModelId("m".into()),
        name: "M".into(),
        points: vec![],
    };
    c.models.push(m.clone());
    c.endpoints = vec![e.clone()];
    c.applied = vec![e];
    let mut values = reset_values(Role::Master, &m);
    values.stale = false;
    values
        .raw
        .insert(PointId("cached".into()), RawValue::Unsigned(55));
    c.values.insert(UnitId("u".into()), values);
    let mut w = Workspace::new(Memory(c));
    run(
        &mut w,
        serde_json::json!({"kind":"resetUnit","id":"e","unit":"u"}),
    )
    .unwrap();
    let c = w.query().catalog;
    assert!(c.values[&UnitId("u".into())].raw.is_empty());
    assert!(c.values[&UnitId("u".into())].stale);
    assert!(run(
        &mut w,
        serde_json::json!({"kind":"resetUnit","id":"e","unit":"missing"})
    )
    .is_err());
}
#[test]
fn inspection_uses_core_protocol_rules_for_drafts_without_changing_authority() {
    let mut c = Catalog::default();
    let e = endpoint();
    let m:CollectionModel=serde_json::from_value(serde_json::json!({"id":"m","name":"M","points":[{"id":"p","name":"P","category":"measurement","area":"holding","address":0,"valueType":"u16","scale":"1","unit":"V","initial":{"kind":"unsigned","value":"12"},"initialEngineering":"12","byteOrder":"big","wordOrder":"big","note":"","slaveWritable":false,"masterPeriodMs":1000}]})).unwrap();
    c.models.push(m.clone());
    c.endpoints.push(e.clone());
    c.applied.push(e.clone());
    c.values
        .insert(UnitId("u".into()), reset_values(Role::Slave, &m));
    let mut draft = e.clone();
    draft.name = "new".into();
    draft.note = "note".into();
    draft.units[0].name = "new unit".into();
    draft.units[0].note = "note".into();
    draft.units[0].device = Some(DeviceId("display".into()));
    c.models[0].name = "new model".into();
    c.models[0].points[0].name = "new point".into();
    c.models[0].points[0].note = "note".into();
    c.models[0].points[0].unit = "显示单位".into();
    let before = c.clone();
    assert!(inspect_endpoint(&c, &draft)[&UnitId("u".into())]);
    assert_eq!(c, before);
    draft.units[0].station = 3;
    assert!(!inspect_endpoint(&c, &draft)[&UnitId("u".into())]);
    draft.units[0].station = 1;
    c.models[0].points[0].address = 1;
    assert!(!inspect_endpoint(&c, &draft)[&UnitId("u".into())]);
}
#[test]
fn master_reapply_keeps_cache_but_marks_it_stale_after_stop() {
    let mut c = Catalog::default();
    let mut e = endpoint();
    e.role = Role::Master;
    let m = CollectionModel {
        id: ModelId("m".into()),
        name: "M".into(),
        points: vec![],
    };
    c.models.push(m.clone());
    c.endpoints.push(e.clone());
    c.applied.push(e);
    let mut v = reset_values(Role::Master, &m);
    v.stale = false;
    c.values.insert(UnitId("u".into()), v);
    let mut w = Workspace::new(Memory(c));
    run(
        &mut w,
        serde_json::json!({"kind":"apply","id":"e","resetIncompatible":false}),
    )
    .unwrap();
    assert!(w.query().catalog.values[&UnitId("u".into())].stale);
}
