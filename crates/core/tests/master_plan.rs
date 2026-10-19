//! 主站请求规划的业务边界。
use essim_core::domain::{
    master::{plan, MasterSettings},
    model::CollectionModel,
};
fn model() -> CollectionModel {
    serde_json::from_value(serde_json::json!({"id":"m","name":"M","points":(0..126).map(|i|serde_json::json!({"id":format!("p{i}"),"name":"P","category":"measurement","area":"holding","address":i,"valueType":"u16","scale":"1","unit":"","initial":{"kind":"unsigned","value":"0"},"initialEngineering":"0","byteOrder":"big","wordOrder":"big","note":"","slaveWritable":false,"masterPeriodMs":1000})).collect::<Vec<_>>() })).unwrap()
}
#[test]
fn groups_stop_at_limits_holes_and_periods() {
    let mut m = model();
    let blocks = plan(&m, &[], false).unwrap();
    assert_eq!(blocks.iter().map(|b| b.count).collect::<Vec<_>>(), [125, 1]);
    m.points.truncate(4);
    m.points[2].address = 10;
    m.points[3].address = 11;
    m.points[3].master_period_ms = 2000;
    let blocks = plan(&m, &[], false).unwrap();
    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[0].count, 2);
}
#[test]
fn wide_points_never_split_and_slave_permission_is_not_master_permission() {
    let mut m = model();
    m.points.truncate(125);
    m.points[124].value_type = essim_core::domain::point_table::ValueType::U32;
    let blocks = plan(&m, &[], false).unwrap();
    assert_eq!(blocks.iter().map(|b| b.count).collect::<Vec<_>>(), [124, 2]);
    let blocks = plan(&m, &[m.points[124].id.clone()], true).unwrap();
    assert_eq!(blocks[0].count, 2);
    assert!(plan(
        &m,
        &[essim_core::domain::identity::PointId("missing".into())],
        false
    )
    .is_err());
    assert!(plan(&m, &[m.points[0].id.clone(), m.points[0].id.clone()], true).is_err());
}
#[test]
fn settings_have_bounded_defaults() {
    let mut s = MasterSettings::default();
    assert_eq!(s.request_timeout_ms, 1000);
    assert!(s.validate().is_ok());
    s.stale_multiplier = 0;
    assert!(s.validate().is_err());
}

#[test]
fn diagnostics_reject_state_changing_subfunctions() {
    use essim_core::domain::master::validate_diagnostic;
    for sub in [1, 3, 4, 10, 20] {
        assert!(validate_diagnostic(8, sub, 0).is_err());
    }
    for sub in [0, 2, 11, 12, 13, 14, 15, 16, 17, 18] {
        assert!(validate_diagnostic(8, sub, 0).is_ok());
    }
    assert!(validate_diagnostic(7, 0, 1).is_err());
    assert!(validate_diagnostic(8, 2, 42).is_err());
    assert!(validate_diagnostic(8, 0, 42).is_ok());
}
