//! 从站模型地址索引、精确解码及原子置数规划。
use essim_core::domain::{
    identity::PointId,
    model::CollectionModel,
    point_table::{decode, encode, Order, RawValue, ValueType},
    slave::ModelLayout,
};
fn model() -> CollectionModel {
    serde_json::from_value(serde_json::json!({"id":"m","name":"M","points":[{"id":"p","name":"P","category":"measurement","area":"holding","address":10,"valueType":"u32","scale":"1","unit":"","initial":{"kind":"unsigned","value":"7"},"initialEngineering":"7","byteOrder":"big","wordOrder":"little","note":"","slaveWritable":true,"masterPeriodMs":1000},{"id":"q","name":"Q","category":"measurement","area":"holding","address":13,"valueType":"u16","scale":"1","unit":"","initial":{"kind":"unsigned","value":"9"},"initialEngineering":"9","byteOrder":"big","wordOrder":"big","note":"","slaveWritable":false,"masterPeriodMs":1000}]})).unwrap()
}
#[test]
fn indexed_span_rejects_holes_and_read_only_points() {
    let layout = ModelLayout::new(model()).unwrap();
    assert!(layout.check_span(2, 10, 2, false).is_ok());
    assert!(layout.check_span(2, 10, 4, false).is_err());
    assert!(layout.check_span(2, 13, 1, true).is_err());
    assert!(layout.check_span(2, 11, 1, true).is_ok());
    assert!(layout.check_span(2, 65535, 2, false).is_err());
}
#[test]
fn batch_is_planned_fully_before_any_write() {
    let layout = ModelLayout::new(model()).unwrap();
    assert!(layout
        .plan_values(&[
            (PointId("p".into()), RawValue::Unsigned(42)),
            (PointId("missing".into()), RawValue::Unsigned(1))
        ])
        .is_err());
    let plan = layout
        .plan_values(&[(PointId("p".into()), RawValue::Unsigned(0x12345678))])
        .unwrap();
    assert_eq!(plan[0].2, vec![0x5678, 0x1234]);
    assert!(layout
        .plan_values(&[
            (PointId("p".into()), RawValue::Unsigned(1)),
            (PointId("p".into()), RawValue::Unsigned(2))
        ])
        .is_err());
}
#[test]
fn wire_decode_round_trips_orders_and_rejects_nonfinite() {
    for (kind, raw) in [
        (ValueType::I16, RawValue::Signed(-123)),
        (ValueType::I32, RawValue::Signed(-123456)),
        (ValueType::U64, RawValue::Unsigned(u64::MAX)),
        (ValueType::F32, RawValue::Float(1.5)),
        (ValueType::F64, RawValue::Float(-0.25)),
    ] {
        for b in [Order::Big, Order::Little] {
            for w in [Order::Big, Order::Little] {
                let words = encode(kind, &raw, b, w).unwrap();
                assert_eq!(decode(kind, &words, b, w).unwrap(), raw);
            }
        }
    }
    assert!(decode(ValueType::F32, &[0x7f80, 0], Order::Big, Order::Big).is_err());
    assert!(decode(ValueType::U32, &[1], Order::Big, Order::Big).is_err());
}
