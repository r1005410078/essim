//! M06纯分组规则与目录原子提交。
use essim_core::{
    application::{
        catalog::Catalog,
        commands::partition::{PartitionRequest, commit_partition, prepare_partition},
    },
    domain::{identity::ModelId, model::CollectionModel},
};
fn model(count: usize) -> CollectionModel {
    serde_json::from_value(serde_json::json!({"id":"source","name":"源模型","points":(0..count).map(|i|serde_json::json!({"id":format!("p{i}"),"name":"点","category":"measurement","area":"holding","address":i*2,"valueType":"u16","scale":"1","unit":"","initial":{"kind":"unsigned","value":"7"},"initialEngineering":"7","byteOrder":"big","wordOrder":"big","note":"","slaveWritable":true,"masterPeriodMs":1000})).collect::<Vec<_>>()})).unwrap()
}
fn request() -> PartitionRequest {
    serde_json::from_value(serde_json::json!({"source":"source","batchId":"batch","prefix":"测试","groupSize":1000,"startIp":"127.0.0.1","port":1080,"station":1,"folder":null})).unwrap()
}
#[test]
fn five_thousand_points_preserve_sparse_addresses_and_create_independent_models() {
    let mut c = Catalog::default();
    c.models.push(model(5000));
    let source = c.models[0].clone();
    let plan = prepare_partition(&c, request()).unwrap();
    assert_eq!(plan.preview().groups.len(), 5);
    let facts = commit_partition(&mut c, plan).unwrap();
    assert_eq!(facts.len(), 5);
    assert_eq!(c.models[0], source);
    assert_eq!(c.models[2].points[0].address, 2000);
    assert_ne!(c.models[2].points[0].id, source.points[1000].id);
    assert_eq!(c.models[2].points.len(), 1000);
    assert!(c.applied.is_empty());
    assert!(c.values.is_empty());
    assert_eq!(c.endpoints[4].units[0].station, 1);
}
#[test]
fn invalid_middle_group_does_not_leave_partial_configuration() {
    let mut c = Catalog::default();
    c.models.push(model(2001));
    let plan = prepare_partition(&c, request()).unwrap();
    let mut changed = c.models[0].clone();
    changed.points[0].name = "已改变".into();
    c.models[0] = changed;
    let before = c.clone();
    assert!(commit_partition(&mut c, plan).is_err());
    assert_eq!(c, before);
}
#[test]
fn wide_point_is_not_split_and_remainder_is_kept() {
    let mut c = Catalog::default();
    let mut m = model(3);
    m.points[1].value_type = serde_json::from_value(serde_json::json!("u32")).unwrap();
    c.models.push(m);
    let mut r = request();
    r.group_size = 2;
    let plan = prepare_partition(&c, r).unwrap();
    assert_eq!(
        plan.preview()
            .groups
            .iter()
            .map(|g| g.points)
            .collect::<Vec<_>>(),
        vec![2, 1]
    );
    commit_partition(&mut c, plan).unwrap();
    assert_eq!(c.models[1].points[1].value_type.width(), 2);
}
#[test]
fn empty_invalid_parameters_overflow_and_conflicts_are_rejected() {
    for count in [0, 2] {
        let mut c = Catalog::default();
        c.models.push(model(count));
        let mut r = request();
        if count > 0 {
            r.group_size = 0;
        }
        assert!(prepare_partition(&c, r).is_err());
    }
    let mut c = Catalog::default();
    c.models.push(model(2));
    let mut r = request();
    r.source = ModelId("missing".into());
    assert!(prepare_partition(&c, r).is_err());
    let mut r = request();
    r.group_size = 1;
    r.start_ip = "255.255.255.255".into();
    assert!(prepare_partition(&c, r).is_err());
    let plan = prepare_partition(&c, request()).unwrap();
    commit_partition(&mut c, plan).unwrap();
    assert!(prepare_partition(&c, request()).is_err());
}
#[test]
fn highest_address_and_mixed_areas_are_ordered_without_reassigning_addresses() {
    let mut c = Catalog::default();
    let mut m = model(3);
    m.points[0].address = 65535;
    m.points[1].slave_writable = false;
    m.points[1].area = serde_json::from_value(serde_json::json!("input")).unwrap();
    m.points[2].area = serde_json::from_value(serde_json::json!("coil")).unwrap();
    m.points[2].value_type = serde_json::from_value(serde_json::json!("bool")).unwrap();
    m.points[2].initial =
        serde_json::from_value(serde_json::json!({"kind":"bool","value":true})).unwrap();
    m.points[2].initial_engineering = "true".into();
    c.models.push(m);
    let mut r = request();
    r.group_size = 2;
    let plan = prepare_partition(&c, r).unwrap();
    commit_partition(&mut c, plan).unwrap();
    assert_eq!(
        c.models[1].points[0].area,
        serde_json::from_value(serde_json::json!("coil")).unwrap()
    );
    assert_eq!(c.models[1].points[1].address, 65535);
}
#[test]
fn repository_failure_publishes_no_event_and_leaves_projection_unchanged() {
    use essim_core::application::{ports::ConfigRepository, service::Workspace};
    use essim_core::domain::error::Error;
    struct Repo(Catalog);
    impl ConfigRepository for Repo {
        fn load(&self) -> Catalog {
            self.0.clone()
        }
        fn commit(&mut self, _: Catalog) -> Result<(), Error> {
            Err(Error::new("STORAGE_FAILED", "注入提交失败"))
        }
    }
    let mut c = Catalog::default();
    c.models.push(model(2001));
    let mut workspace = Workspace::new(Repo(c.clone()));
    let plan = workspace.prepare_partition(request()).unwrap();
    assert!(workspace.commit_partition(plan, "cmd".into()).is_err());
    assert_eq!(workspace.sequence(), 0);
    assert_eq!(workspace.query().catalog, c);
}

#[test]
fn invalid_source_and_removed_folder_leave_catalog_unchanged() {
    let mut c = Catalog::default();
    let mut m = model(2);
    m.points[1].address = m.points[0].address;
    c.models.push(m);
    assert!(prepare_partition(&c, request()).is_err());
    c.models[0] = model(2001);
    let mut r = request();
    r.folder = Some(essim_core::domain::identity::FolderId("gone".into()));
    assert!(prepare_partition(&c, r).is_err());
    let plan = prepare_partition(&c, request()).unwrap();
    commit_partition(&mut c, plan).unwrap();
    let before = c.models[0].clone();
    let other = c.models[2].clone();
    c.models[1].points[0].initial_engineering = "123".into();
    assert_eq!(c.models[0], before);
    assert_ne!(
        c.models[1].points[0].initial_engineering,
        other.points[0].initial_engineering
    );
}

#[test]
fn removed_folder_and_middle_identity_conflict_leave_no_generated_objects() {
    use essim_core::domain::{
        aggregates::navigation::Folder,
        identity::{FolderId, PointId},
    };
    let mut c = Catalog::default();
    c.models.push(model(2001));
    let folder = FolderId("target".into());
    c.navigation
        .save_folder(Folder {
            id: folder.clone(),
            name: "目标".into(),
            parent: None,
        })
        .unwrap();
    let mut r = request();
    r.folder = Some(folder.clone());
    let plan = prepare_partition(&c, r).unwrap();
    c.navigation.delete_folder(&folder).unwrap();
    let before = c.clone();
    assert!(commit_partition(&mut c, plan).is_err());
    assert_eq!(c, before);
    c.models[0].points[0].id = PointId("batch:group:1:model:point:0".into());
    let plan = prepare_partition(&c, request()).unwrap();
    let before = c.clone();
    assert!(commit_partition(&mut c, plan).is_err());
    assert_eq!(c, before);
}

#[test]
fn invalid_network_and_station_parameters_are_rejected_before_creation() {
    let mut c = Catalog::default();
    c.models.push(model(2));
    for ip in [
        "not-an-ip",
        "::1",
        "0.0.0.0",
        "224.0.0.1",
        "240.0.0.1",
        "255.255.255.255",
    ] {
        let mut r = request();
        r.start_ip = ip.into();
        assert!(prepare_partition(&c, r).is_err());
    }
    for station in [0, 248, 255] {
        let mut r = request();
        r.station = station;
        assert!(prepare_partition(&c, r).is_err());
    }
    let mut r = request();
    r.port = 0;
    assert!(prepare_partition(&c, r).is_err());
    assert_eq!(c.models.len(), 1);
    assert!(c.endpoints.is_empty());
}
