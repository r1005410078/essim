//! 模型覆盖保存、复制和引用保护；无真实协议资源。
use essim_core::{application::workspace::*, domain::foundation::*};

#[derive(Default)]
struct Memory {
    catalog: Catalog,
}
impl ConfigRepository for Memory {
    fn load(&self) -> Catalog {
        self.catalog.clone()
    }
    fn commit(&mut self, value: Catalog) -> Result<(), Error> {
        self.catalog = value;
        Ok(())
    }
}
fn execute(w: &mut Workspace<Memory>, action: Action) -> Result<Receipt, Error> {
    w.execute(Command {
        command_id: "test".into(),
        action,
    })
    .map(|v| v.0)
}
fn model() -> CollectionModel {
    CollectionModel {
        id: ModelId("a".into()),
        name: "A".into(),

        points: vec![PointDefinition {
            id: PointId("b:point:0:0".into()),
            name: "点".into(),
            category: Telemetry::Measurement,
            area: Area::Holding,
            address: 0,
            value_type: ValueType::U16,
            scale: "1".into(),
            unit: String::new(),
            initial: RawValue::Unsigned(0),
            initial_engineering: "0".into(),
            byte_order: Order::Big,
            word_order: Order::Big,
            note: String::new(),
            slave_writable: false,
            master_period_ms: 1000,
        }],
    }
}
#[test]
fn saves_replace_current_model_and_copies_remain_independent() {
    let mut w = Workspace::new(Memory::default());
    execute(&mut w, Action::SaveModel { model: model() }).unwrap();
    execute(
        &mut w,
        Action::CopyModel {
            source: ModelRef {
                id: ModelId("a".into()),
            },
            id: ModelId("b".into()),
            name: "B".into(),
        },
    )
    .unwrap();
    let mut changed = model();
    changed.name = "A2".into();
    execute(&mut w, Action::SaveModel { model: changed }).unwrap();
    let catalog = w.query().catalog;
    assert_eq!(catalog.models.len(), 2);
    let current = catalog.models.iter().find(|m| m.id.0 == "a").unwrap();
    let copy = catalog.models.iter().find(|m| m.id.0 == "b").unwrap();
    assert_eq!(current.name, "A2");
    assert_eq!(copy.name, "B");
    assert_ne!(current.points[0].id, copy.points[0].id);
    let json = serde_json::to_value(current).unwrap();
    assert!(json.get("version").is_none());
    execute(
        &mut w,
        Action::DeleteModel {
            id: ModelId("a".into()),
        },
    )
    .unwrap();
    assert_eq!(w.query().catalog.models.len(), 1);
}
#[test]
fn old_applied_reference_survives_new_saved_configuration() {
    let mut memory = Memory::default();
    memory.catalog.models.push(model());
    let endpoint = EndpointConfig {
        id: EndpointId("e".into()),
        name: "E".into(),
        note: String::new(),

        role: Role::Slave,
        transport: Transport::Tcp {
            address: "127.0.0.1".into(),
            port: 1502,
        },
        units: vec![UnitConfig {
            id: UnitId("u".into()),
            name: "U".into(),
            note: String::new(),
            station: 1,
            enabled: true,
            device: None,
            model: ModelRef {
                id: ModelId("a".into()),
            },
        }],
    };
    memory.catalog.applied.push(endpoint);
    let mut w = Workspace::new(memory);
    assert_eq!(
        w.query()
            .catalog
            .model_references(&ModelId("a".into()))
            .len(),
        1
    );
    assert_eq!(
        execute(
            &mut w,
            Action::DeleteModel {
                id: ModelId("a".into()),
            }
        )
        .unwrap_err()
        .code,
        "MODEL_REFERENCED"
    );
}
#[test]
fn deletion_rechecks_references_created_after_a_read_only_preview() {
    let mut w = Workspace::new(Memory::default());
    execute(&mut w, Action::SaveModel { model: model() }).unwrap();
    assert!(w
        .query()
        .catalog
        .model_references(&ModelId("a".into()))
        .is_empty());
    let endpoint = EndpointConfig {
        id: EndpointId("new-e".into()),
        name: "E".into(),
        note: String::new(),

        role: Role::Slave,
        transport: Transport::Tcp {
            address: "127.0.0.1".into(),
            port: 1502,
        },
        units: vec![UnitConfig {
            id: UnitId("new-u".into()),
            name: "U".into(),
            note: String::new(),
            station: 1,
            enabled: true,
            device: None,
            model: ModelRef {
                id: ModelId("a".into()),
            },
        }],
    };
    execute(&mut w, Action::CreateEndpoint { endpoint }).unwrap();
    let before = w.query().catalog;
    assert_eq!(
        execute(
            &mut w,
            Action::DeleteModel {
                id: ModelId("a".into()),
            }
        )
        .unwrap_err()
        .code,
        "MODEL_REFERENCED"
    );
    assert_eq!(w.query().catalog, before);
}

#[test]
fn overwrite_keeps_applied_snapshot_until_explicit_apply() {
    let mut w = Workspace::new(Memory::default());
    execute(&mut w, Action::SaveModel { model: model() }).unwrap();
    let endpoint = EndpointConfig {
        id: EndpointId("e".into()),
        name: "E".into(),
        note: String::new(),
        role: Role::Slave,
        transport: Transport::Tcp {
            address: "127.0.0.1".into(),
            port: 1502,
        },
        units: vec![UnitConfig {
            id: UnitId("u".into()),
            name: "U".into(),
            note: String::new(),
            station: 1,
            enabled: true,
            device: None,
            model: ModelRef {
                id: ModelId("a".into()),
            },
        }],
    };
    execute(
        &mut w,
        Action::CreateEndpoint {
            endpoint: endpoint.clone(),
        },
    )
    .unwrap();
    execute(
        &mut w,
        Action::Apply {
            id: endpoint.id.clone(),
            reset_incompatible: false,
        },
    )
    .unwrap();
    let original = w.query().catalog.values;
    let mut changed = model();
    changed.name = "覆盖名称".into();
    changed.points[0].address = 100;
    execute(
        &mut w,
        Action::SaveModel {
            model: changed.clone(),
        },
    )
    .unwrap();
    let saved = w.query().catalog;
    assert_eq!(saved.models, vec![changed.clone()]);
    assert_eq!(saved.values, original);
    assert_eq!(saved.endpoints[0].units[0].model.id.0, "a");
    execute(
        &mut w,
        Action::Apply {
            id: endpoint.id.clone(),
            reset_incompatible: false,
        },
    )
    .unwrap();
    assert_eq!(
        w.query().catalog.values[&UnitId("u".into())].model,
        Some(changed)
    );
    let serialized = serde_json::to_value(&w.query().catalog).unwrap();
    assert!(serialized["endpoints"][0].get("revision").is_none());
    assert!(serialized["endpoints"][0].get("appliedRevision").is_none());
    assert!(serialized["endpoints"][0]["units"][0]["model"]
        .get("version")
        .is_none());
}
