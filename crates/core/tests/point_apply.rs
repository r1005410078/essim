//! 停机边界使用可控适配器；不构成真实Modbus停机或互操作证据。
use essim_core::{application::workspace::*, domain::foundation::*};
use std::{cell::RefCell, rc::Rc};
#[derive(Clone)]
struct Memory(Rc<RefCell<Catalog>>);
impl ConfigRepository for Memory {
    fn load(&self) -> Catalog {
        self.0.borrow().clone()
    }
    fn commit(&mut self, value: Catalog) -> Result<(), Error> {
        *self.0.borrow_mut() = value;
        Ok(())
    }
}
struct Boundary {
    memory: Memory,
    calls: Rc<RefCell<Vec<Vec<UnitId>>>>,
    conflict: bool,
    fail: bool,
}
impl ApplyBoundary for Boundary {
    fn stop(&mut self, e: &EndpointConfig) -> Result<(), Error> {
        self.calls
            .borrow_mut()
            .push(e.units.iter().map(|u| u.id.clone()).collect());
        if self.fail {
            return Err(Error::new("STOP_FAILED", "未确认停止"));
        }
        if self.conflict {
            self.memory.0.borrow_mut().endpoints[0].name = "停机时改变".into();
        }
        Ok(())
    }
}
fn model(_sample: u64) -> CollectionModel {
    let scale = if _sample == 1 { "1" } else { "0.1" };
    CollectionModel {
        id: ModelId("m".into()),
        name: "M".into(),

        points: vec![PointDefinition {
            id: PointId("p".into()),
            name: "P".into(),
            category: Telemetry::Measurement,
            area: Area::Holding,
            address: 0,
            value_type: ValueType::U16,
            byte_order: Order::Big,
            word_order: Order::Big,
            scale: scale.into(),
            unit: "V".into(),
            initial: RawValue::Unsigned(0),
            initial_engineering: "0".into(),
            note: String::new(),
            slave_writable: false,
            master_period_ms: 1000,
        }],
    }
}
fn fixture(
    conflict: bool,
    fail: bool,
    role: Role,
    previous_role: Role,
) -> (Workspace<Memory, Boundary>, Rc<RefCell<Vec<Vec<UnitId>>>>) {
    let old = model(1);
    let mut next = model(2);
    next.id = ModelId("changed".into());
    let mut previous_changed = old.clone();
    previous_changed.id = next.id.clone();
    let mut e = EndpointConfig {
        master: Default::default(),
        id: EndpointId("tcp".into()),
        name: "E".into(),
        note: String::new(),

        role: Role::Slave,
        transport: Transport::Tcp {
            address: "127.0.0.1".into(),
            port: 1502,
        },
        units: vec![],
    };
    for station in 1..=3 {
        e.units.push(UnitConfig {
            overrides: Default::default(),
id: UnitId(format!("u{station}")),
            name: format!("U{station}"),
        note: String::new(),
            station,
            enabled: true,
            device: None,
            model: ModelRef {
                id: if station == 2 {
                    next.id.clone()
                } else {
                    old.id.clone()
                },
            },
        });
    }
    let mut catalog = Catalog::default();
    catalog.models = vec![old.clone(), next];
    e.role = previous_role;
    catalog.applied.push(e.clone());
    for u in &e.units {
        let mut values = reset_values(
            Role::Slave,
            if u.station == 2 {
                &previous_changed
            } else {
                &old
            },
        );
        values.raw.insert(
            PointId("p".into()),
            RawValue::Unsigned(100 * u.station as u64),
        );
        catalog.values.insert(u.id.clone(), values);
    }
    e.role = role;
    catalog.endpoints.push(e);
    let memory = Memory(Rc::new(RefCell::new(catalog)));
    let calls = Rc::new(RefCell::new(vec![]));
    let boundary = Boundary {
        memory: memory.clone(),
        calls: calls.clone(),
        conflict,
        fail,
    };
    (Workspace::with_boundary(memory, boundary), calls)
}
fn apply(w: &mut Workspace<Memory, Boundary>) -> Result<(Receipt, ConfigEvent), Error> {
    w.execute(Command {
        command_id: "apply".into(),
        action: Action::Apply {
            id: EndpointId("tcp".into()),

            reset_incompatible: false,
        },
    })
}
#[test]
fn preview_is_pure_and_apply_stops_all_units_preserving_others() {
    let (mut w, calls) = fixture(false, false, Role::Slave, Role::Slave);
    let before = w.query().catalog;
    let preview = w.preview_apply(&EndpointId("tcp".into()), false).unwrap();
    assert_eq!(w.query().catalog, before);
    assert!(calls.borrow().is_empty());
    let changed = &preview.units[&UnitId("u2".into())].engineering_changed[0];
    assert_eq!(
        (&changed.previous, &changed.next),
        (&"200".to_string(), &"20".to_string())
    );
    apply(&mut w).unwrap();
    assert_eq!(
        calls.borrow()[0],
        vec![
            UnitId("u1".into()),
            UnitId("u2".into()),
            UnitId("u3".into())
        ]
    );
    let after = w.query().catalog;
    assert_eq!(
        before.values[&UnitId("u1".into())].raw,
        after.values[&UnitId("u1".into())].raw
    );
    assert_eq!(
        before.values[&UnitId("u3".into())].raw,
        after.values[&UnitId("u3".into())].raw
    );
    assert_eq!(
        after.values[&UnitId("u2".into())].raw[&PointId("p".into())],
        RawValue::Unsigned(200)
    );
    assert_eq!(
        w.application_state(&EndpointId("tcp".into())),
        Some(ApplyState::Stopped)
    );
    assert!(!w.query().capabilities.execution);
}
#[test]
fn conflict_after_stop_keeps_old_application_and_records_stopped_state() {
    let (mut w, _) = fixture(true, false, Role::Slave, Role::Slave);
    let before = w.query().catalog;
    assert_eq!(apply(&mut w).unwrap_err().code, "CONFIG_CHANGED");
    assert_eq!(w.query().catalog.endpoints[0].name, "停机时改变");
    assert_eq!(w.query().catalog.applied, before.applied);
    assert_eq!(w.query().catalog.values, before.values);
    assert_eq!(
        w.application_state(&EndpointId("tcp".into())),
        Some(ApplyState::Stopped)
    );
}
#[test]
fn unconfirmed_stop_never_changes_application() {
    let (mut w, _) = fixture(false, true, Role::Slave, Role::Slave);
    let before = w.query().catalog;
    assert_eq!(apply(&mut w).unwrap_err().code, "STOP_FAILED");
    assert_eq!(w.query().catalog, before);
    assert_eq!(w.application_state(&EndpointId("tcp".into())), Some(ApplyState::Failed));
}

#[test]
fn master_invalidates_only_changed_mapping_and_never_invents_remote_initials() {
    let (mut w, _) = fixture(false, false, Role::Master, Role::Master);
    let before = w.query().catalog;
    apply(&mut w).unwrap();
    let after = w.query().catalog;
    assert_eq!(
        after.values[&UnitId("u1".into())].raw,
        before.values[&UnitId("u1".into())].raw
    );
    assert!(after.values[&UnitId("u1".into())].stale);
    assert!(after.values[&UnitId("u2".into())].raw.is_empty());
    assert!(after.values[&UnitId("u2".into())].stale);
}

#[test]
fn a_local_slave_value_never_becomes_a_master_remote_read_result() {
    let (mut w, _) = fixture(false, false, Role::Master, Role::Slave);
    apply(&mut w).unwrap();
    assert!(w
        .query()
        .catalog
        .values
        .values()
        .all(|v| v.raw.is_empty() && v.stale));
}
