//! 模型复制、版本和引用保护；无真实协议资源。
use essim_core::{application::workspace::*, domain::foundation::*};

#[derive(Default)]
struct Memory {
    catalog: Catalog,
    retained: Vec<ModelRef>,
}
impl ConfigRepository for Memory {
    fn load(&self) -> Catalog {
        self.catalog.clone()
    }
    fn commit(&mut self, value: Catalog) -> Result<(), Error> {
        self.catalog = value;
        Ok(())
    }
    fn retained_models(&self) -> Vec<ModelRef> {
        self.retained.clone()
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
        version: 1,
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
fn copies_are_independent_and_old_versions_are_immutable() {
    let mut w = Workspace::new(Memory::default());
    execute(
        &mut w,
        Action::SaveModel {
            model: model(),
            expected_revision: 0,
        },
    )
    .unwrap();
    execute(
        &mut w,
        Action::CopyModel {
            source: ModelRef {
                id: ModelId("a".into()),
                version: 1,
            },
            id: ModelId("b".into()),
            name: "B".into(),
            expected_revision: 1,
        },
    )
    .unwrap();
    let mut renamed = model();
    renamed.name = "A2".into();
    execute(
        &mut w,
        Action::SaveModel {
            model: renamed,
            expected_revision: 1,
        },
    )
    .unwrap();
    let catalog = w.query().catalog;
    assert_eq!(catalog.models[0].name, "A");
    assert_eq!(catalog.models[1].id, ModelId("b".into()));
    assert_ne!(
        catalog.models[0].points[0].id,
        catalog.models[1].points[0].id
    );
    assert_eq!(
        catalog.models[0].points[0].id,
        catalog.models[2].points[0].id
    );
    assert_eq!(catalog.models[2].version, 2);
    assert!(execute(
        &mut w,
        Action::DeleteModel {
            id: ModelId("a".into()),
            expected_revision: 1
        }
    )
    .is_err());
    execute(
        &mut w,
        Action::DeleteModel {
            id: ModelId("a".into()),
            expected_revision: 2,
        },
    )
    .unwrap();
    assert!(w.query().catalog.models.iter().all(|m| m.id.0 == "b"));
}
#[test]
fn recorded_version_protects_model_even_without_configuration_reference() {
    let mut memory = Memory::default();
    memory.catalog.models.push(model());
    memory.retained.push(ModelRef {
        id: ModelId("a".into()),
        version: 1,
    });
    let mut w = Workspace::new(memory);
    let before = w.query().catalog;
    let error = execute(
        &mut w,
        Action::DeleteModel {
            id: ModelId("a".into()),
            expected_revision: 1,
        },
    )
    .unwrap_err();
    assert_eq!(error.code, "MODEL_REFERENCED");
    assert_eq!(w.query().catalog, before);
}
#[test]
fn old_applied_reference_survives_new_saved_configuration() {
    let mut memory = Memory::default();
    memory.catalog.models.push(model());
    let endpoint = EndpointConfig {
        id: EndpointId("e".into()),
        name: "E".into(),
        revision: 1,
        applied_revision: Some(1),
        role: Role::Slave,
        transport: Transport::Tcp {
            address: "127.0.0.1".into(),
            port: 1502,
        },
        units: vec![UnitConfig {
            id: UnitId("u".into()),
            name: "U".into(),
            station: 1,
            enabled: true,
            device: None,
            model: ModelRef {
                id: ModelId("a".into()),
                version: 1,
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
                expected_revision: 1
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
    execute(
        &mut w,
        Action::SaveModel {
            model: model(),
            expected_revision: 0,
        },
    )
    .unwrap();
    assert!(w
        .query()
        .catalog
        .model_references(&ModelId("a".into()))
        .is_empty());
    let endpoint = EndpointConfig {
        id: EndpointId("new-e".into()),
        name: "E".into(),
        revision: 0,
        applied_revision: None,
        role: Role::Slave,
        transport: Transport::Tcp {
            address: "127.0.0.1".into(),
            port: 1502,
        },
        units: vec![UnitConfig {
            id: UnitId("new-u".into()),
            name: "U".into(),
            station: 1,
            enabled: true,
            device: None,
            model: ModelRef {
                id: ModelId("a".into()),
                version: 1,
            },
        }],
    };
    execute(
        &mut w,
        Action::SaveEndpoint {
            endpoint,
            expected_revision: 0,
        },
    )
    .unwrap();
    let before = w.query().catalog;
    assert_eq!(
        execute(
            &mut w,
            Action::DeleteModel {
                id: ModelId("a".into()),
                expected_revision: 1
            }
        )
        .unwrap_err()
        .code,
        "MODEL_REFERENCED"
    );
    assert_eq!(w.query().catalog, before);
}
