//! M01 配置不变量；测试中的名称与地址仅是测试样本。
use essim_core::domain::foundation::*;

fn endpoint(id: &str) -> EndpointConfig {
    EndpointConfig {
        master: Default::default(),
        id: EndpointId(id.into()),
        name: id.into(),
        note: String::new(),

        role: Role::Slave,
        transport: Transport::Tcp {
            address: "127.0.0.1".into(),
            port: 1502,
        },
        units: vec![],
    }
}
fn unit(id: &str, station: u8) -> UnitConfig {
    UnitConfig {
        id: UnitId(id.into()),
        name: id.into(),
        note: String::new(),
        station,
        enabled: true,
        model: ModelRef {
            id: ModelId("model-a".into()),
        },
        device: None,
    }
}
#[test]
fn station_change_keeps_identity_and_rejects_duplicates_atomically() {
    let mut endpoint = endpoint("endpoint-a");
    endpoint.units = vec![unit("unit-a", 1), unit("unit-b", 2)];
    let mut changed = endpoint.clone();
    changed.units[0].station = 2;
    assert!(changed.validate().is_err());
    assert_eq!(endpoint.units[0].station, 1);
    changed.units[0].station = 3;
    assert!(changed.validate().is_ok());
    assert_eq!(changed.units[0].id, endpoint.units[0].id);
    let mut other = endpoint.clone();
    other.id = EndpointId("endpoint-b".into());
    assert!(other.validate().is_ok());
}
#[test]
fn rtu_slave_is_single_station_but_master_can_have_multiple_targets() {
    let mut endpoint = endpoint("serial-a");
    endpoint.transport = Transport::Rtu {
        path: "/dev/test-only".into(),
        baud: 19200,
        parity: Parity::None,
        data_bits: 8,
        stop_bits: 1,
    };
    endpoint.units = vec![unit("unit-a", 1), unit("unit-b", 2)];
    assert!(endpoint.validate().is_err());
    endpoint.role = Role::Master;
    assert!(endpoint.validate().is_ok());
}

use essim_core::application::workspace::*;
#[derive(Default)]
struct Memory(Catalog);
impl ConfigRepository for Memory {
    fn load(&self) -> Catalog {
        self.0.clone()
    }
    fn commit(&mut self, catalog: Catalog) -> Result<(), Error> {
        self.0 = catalog;
        Ok(())
    }
}
fn command(action: Action) -> Command {
    Command {
        command_id: "test-command".into(),
        action,
    }
}
fn model(_sample: u64) -> CollectionModel {
    CollectionModel {
        id: ModelId("model-a".into()),
        name: "测试表".into(),

        points: vec![],
    }
}
fn save_model(workspace: &mut Workspace<Memory>) -> ConfigEvent {
    workspace
        .execute(command(Action::SaveModel { model: model(1) }))
        .unwrap()
        .1
}
#[test]
fn save_is_not_apply_and_bad_references_never_mutate() {
    let mut workspace = Workspace::new(Memory::default());
    save_model(&mut workspace);
    let mut config = endpoint("endpoint-a");
    config.units = vec![unit("unit-a", 1)];
    if !workspace
        .query()
        .catalog
        .endpoints
        .iter()
        .any(|e| e.id == config.id)
    {
        let mut first = config.clone();
        first.units.truncate(1);
        workspace
            .execute(command(Action::CreateEndpoint { endpoint: first }))
            .unwrap();
    }
    workspace
        .execute(command(Action::SaveEndpoint {
            endpoint: config.clone(),
        }))
        .unwrap();
    assert!(workspace.query().catalog.applied.is_empty());
    let before = workspace.query();
    config.units[0].model.id = ModelId("missing".into());
    assert!(workspace
        .execute(command(Action::SaveEndpoint { endpoint: config }))
        .is_err());
    assert_eq!(before.catalog, workspace.query().catalog);
    assert_eq!(before.sequence, workspace.query().sequence);
    workspace
        .execute(command(Action::Apply {
            id: EndpointId("endpoint-a".into()),

            reset_incompatible: false,
        }))
        .unwrap();
    assert_eq!(workspace.query().catalog.applied[0].id.0, "endpoint-a");
    assert!(!workspace.query().capabilities.execution);
    assert!(workspace
        .execute(command(Action::Start {
            id: EndpointId("endpoint-a".into())
        }))
        .is_err());
}
#[test]
fn projection_dedupe_gap_and_staleness_do_not_control_authority() {
    let mut workspace = Workspace::new(Memory::default());
    let event = save_model(&mut workspace);
    let mut projection = Projection::default();
    assert!(projection.query().catalog.models.is_empty());
    assert!(projection.accept(&event).unwrap());
    assert!(!projection.accept(&event).unwrap());
    let mut gap = event.clone();
    gap.sequence = 3;
    assert!(projection.accept(&gap).is_err());
    gap.sequence = 2;
    gap.schema = 9;
    assert!(projection.accept(&gap).is_err());
    assert_eq!(projection.query().sequence, 1);
}
fn point(id: &str) -> PointDefinition {
    PointDefinition {
        id: PointId(id.into()),
        name: id.into(),
        category: Telemetry::Measurement,
        area: Area::Holding,
        address: if id == "added" {
            200
        } else if id == "removed" {
            300
        } else {
            0
        },
        value_type: ValueType::U16,
        scale: "0.1".into(),
        unit: "V".into(),
        initial: RawValue::Unsigned(10),
        initial_engineering: "1".into(),
        byte_order: Order::Big,
        word_order: Order::Big,
        note: String::new(),
        slave_writable: false,
        master_period_ms: 1000,
    }
}
#[test]
fn migration_keeps_raw_across_scale_and_address_changes_and_is_atomic() {
    let mut old = model(1);
    old.points = vec![point("keep"), point("removed")];
    let current = UnitValues {
        model: Some(old.clone()),
        raw: [
            (PointId("keep".into()), RawValue::Unsigned(55)),
            (PointId("removed".into()), RawValue::Unsigned(22)),
        ]
        .into(),
        stale: false,
    };
    let mut next = old.clone();

    next.points.pop();
    next.points[0].scale = "1".into();
    next.points[0].initial_engineering = "10".into();
    next.points[0].address = 100;
    next.points.push(point("added"));
    let preview = migrate(Some(&old), &current, &next, false).unwrap();
    assert_eq!(
        preview.values.raw[&PointId("keep".into())],
        RawValue::Unsigned(55)
    );
    assert_eq!(
        preview.values.raw[&PointId("added".into())],
        RawValue::Unsigned(10)
    );
    assert_eq!(preview.removed, vec![PointId("removed".into())]);
    assert_eq!(preview.scale_changed, vec![PointId("keep".into())]);
    next.points[0].value_type = ValueType::Bool;
    next.points[0].area = Area::Coil;
    next.points[0].initial = RawValue::Bool(false);
    next.points[0].initial_engineering = "false".into();
    assert!(migrate(Some(&old), &current, &next, false).is_err());
    assert_eq!(current.raw[&PointId("keep".into())], RawValue::Unsigned(55));
    assert_eq!(
        migrate(Some(&old), &current, &next, true)
            .unwrap()
            .values
            .raw[&PointId("keep".into())],
        RawValue::Bool(false)
    );
}
#[test]
fn apply_all_units_is_atomic_but_batch_reports_each_endpoint() {
    let mut workspace = Workspace::new(Memory::default());
    let mut v1 = model(1);
    v1.points = vec![point("p")];
    workspace
        .execute(command(Action::SaveModel { model: v1.clone() }))
        .unwrap();
    let mut config = endpoint("endpoint-a");
    config.units = vec![unit("unit-a", 1), unit("unit-b", 2)];
    if !workspace
        .query()
        .catalog
        .endpoints
        .iter()
        .any(|e| e.id == config.id)
    {
        let mut first = config.clone();
        first.units.truncate(1);
        workspace
            .execute(command(Action::CreateEndpoint { endpoint: first }))
            .unwrap();
    }
    workspace
        .execute(command(Action::SaveEndpoint {
            endpoint: config.clone(),
        }))
        .unwrap();
    workspace
        .execute(command(Action::Apply {
            id: config.id.clone(),

            reset_incompatible: false,
        }))
        .unwrap();
    let mut v2 = v1;
    v2.points[0].value_type = ValueType::Bool;
    v2.points[0].area = Area::Coil;
    v2.points[0].initial = RawValue::Bool(false);
    v2.points[0].initial_engineering = "false".into();
    v2.points[0].scale = "1".into();
    workspace
        .execute(command(Action::SaveModel { model: v2 }))
        .unwrap();
    if !workspace
        .query()
        .catalog
        .endpoints
        .iter()
        .any(|e| e.id == config.id)
    {
        let mut first = config.clone();
        first.units.truncate(1);
        workspace
            .execute(command(Action::CreateEndpoint { endpoint: first }))
            .unwrap();
    }
    workspace
        .execute(command(Action::SaveEndpoint {
            endpoint: config.clone(),
        }))
        .unwrap();
    let before = workspace.query().catalog;
    assert!(workspace
        .execute(command(Action::Apply {
            id: config.id.clone(),

            reset_incompatible: false
        }))
        .is_err());
    assert_eq!(workspace.query().catalog, before);
    let results = workspace.execute_batch(vec![
        command(Action::Apply {
            id: config.id,

            reset_incompatible: true,
        }),
        command(Action::Apply {
            id: EndpointId("missing".into()),

            reset_incompatible: false,
        }),
    ]);
    assert!(results[0].is_ok());
    assert!(results[1].is_err());
    assert_eq!(workspace.query().catalog.models.len(), 1);
}
#[test]
fn independent_models_optional_devices_and_transport_validation() {
    let mut workspace = Workspace::new(Memory::default());
    save_model(&mut workspace);
    let mut other = model(1);
    other.id = ModelId("model-b".into());
    other.name = "测试表B".into();
    workspace
        .execute(command(Action::SaveModel { model: other }))
        .unwrap();
    let device = Device {
        id: DeviceId("device-a".into()),
        name: "PCS".into(),
        kind: "PCS".into(),
    };
    workspace
        .execute(command(Action::CreateDevice {
            device: device.clone(),
        }))
        .unwrap();
    let mut config = endpoint("endpoint-a");
    config.units = vec![unit("a", 1), unit("b", 2)];
    config.units[1].model.id = ModelId("model-b".into());
    config.units[0].device = Some(device.id);
    if !workspace
        .query()
        .catalog
        .endpoints
        .iter()
        .any(|e| e.id == config.id)
    {
        let mut first = config.clone();
        first.units.truncate(1);
        workspace
            .execute(command(Action::CreateEndpoint { endpoint: first }))
            .unwrap();
    }
    workspace
        .execute(command(Action::SaveEndpoint {
            endpoint: config.clone(),
        }))
        .unwrap();
    assert_ne!(
        workspace.query().catalog.endpoints[0].units[0].model,
        workspace.query().catalog.endpoints[0].units[1].model
    );
    config.units[0].device = None;
    if !workspace
        .query()
        .catalog
        .endpoints
        .iter()
        .any(|e| e.id == config.id)
    {
        let mut first = config.clone();
        first.units.truncate(1);
        workspace
            .execute(command(Action::CreateEndpoint { endpoint: first }))
            .unwrap();
    }
    workspace
        .execute(command(Action::SaveEndpoint {
            endpoint: config.clone(),
        }))
        .unwrap();
    assert!(workspace.query().catalog.endpoints[0].units[0]
        .device
        .is_none());
    config.transport = Transport::Tcp {
        address: "not-ip".into(),
        port: 0,
    };
    assert!(config.validate().is_err());
    config.transport = Transport::Rtu {
        path: String::new(),
        baud: 1,
        parity: Parity::None,
        data_bits: 7,
        stop_bits: 0,
    };
    assert!(config.validate().is_err());
}
#[test]
fn explicit_reset_uses_applied_initial_values_and_master_only_clears_cache() {
    let mut model = model(1);
    model.points = vec![point("p")];
    assert_eq!(
        reset_values(Role::Slave, &model).raw[&PointId("p".into())],
        RawValue::Unsigned(10)
    );
    let master = reset_values(Role::Master, &model);
    assert!(master.raw.is_empty());
    assert!(master.stale);
}
