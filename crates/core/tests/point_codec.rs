//! M02已知数值向量与十进制边界；示例地址仅用于测试。
use essim_core::domain::foundation::{
    Area, CollectionModel, ModelId, PointDefinition, PointId, Telemetry,
};
use essim_core::domain::point_table::*;

fn definition(id: &str, address: u16, kind: ValueType) -> PointDefinition {
    let engineering = if kind == ValueType::Bool {
        "false"
    } else {
        "0"
    };
    PointDefinition {
        id: PointId(id.into()),
        name: id.into(),
        address,
        value_type: kind,
        area: if kind == ValueType::Bool {
            Area::Coil
        } else {
            Area::Holding
        },
        category: Telemetry::Measurement,
        scale: "1".into(),
        unit: String::new(),
        initial_engineering: engineering.into(),
        initial: from_engineering(kind, engineering, "1").unwrap(),
        byte_order: Order::Big,
        word_order: Order::Big,
        note: String::new(),
        slave_writable: false,
        master_period_ms: 1000,
    }
}

#[test]
fn full_table_validation_crosses_categories_and_reports_stable_identity() {
    let mut first = definition("a", 100, ValueType::F32);
    let mut second = definition("b", 101, ValueType::U16);
    second.category = Telemetry::Indication;
    let model = |points| CollectionModel {
        id: ModelId("model".into()),
        name: "测试".into(),

        points,
    };
    let error = model(vec![first.clone(), second.clone()])
        .validate()
        .unwrap_err();
    assert_eq!(error.code, "ADDRESS_OVERLAP");
    assert_eq!(error.point_id.as_deref(), Some("b"));
    assert_eq!(error.related_point.as_deref(), Some("a"));
    second.area = Area::Input;
    model(vec![first.clone(), second.clone()])
        .validate()
        .unwrap();
    first.address = 65535;
    assert_eq!(
        model(vec![first]).validate().unwrap_err().code,
        "ADDRESS_OVERFLOW"
    );
    second.category = Telemetry::Control;
    let error = model(vec![second]).validate().unwrap_err();
    assert_eq!(error.field.as_deref(), Some("area"));
}

#[test]
fn dto_preserves_64_bits_and_rejects_forged_initial_values() {
    let raw = RawValue::Unsigned(u64::MAX);
    let json = serde_json::to_string(&raw).unwrap();
    assert!(json.contains("\"18446744073709551615\""));
    assert_eq!(serde_json::from_str::<RawValue>(&json).unwrap(), raw);
    assert!(serde_json::from_str::<RawValue>(
        r#"{"kind":"unsigned","value":18446744073709551615}"#
    )
    .is_err());
    let mut point = definition("a", 0, ValueType::U16);
    point.initial = RawValue::Unsigned(10);
    assert_eq!(
        point.validate().unwrap_err().field.as_deref(),
        Some("initial")
    );
}

#[test]
fn decimal_conversion_is_exact_and_never_truncates() {
    assert_eq!(
        from_engineering(ValueType::I16, "0.3", "0.1").unwrap(),
        RawValue::Signed(3)
    );
    assert!(from_engineering(ValueType::I16, "12.35", "0.1")
        .unwrap_err()
        .message
        .contains("12.3"));
    assert_eq!(
        from_engineering(ValueType::I16, "-1.5", "-0.1").unwrap(),
        RawValue::Signed(15)
    );
    for scale in ["0", "NaN", "inf", "1e9999"] {
        assert!(from_engineering(ValueType::U16, "1", scale).is_err());
    }
}

#[test]
fn integer_boundaries_are_not_converted_through_float() {
    assert_eq!(
        from_engineering(ValueType::U64, "18446744073709551615", "1").unwrap(),
        RawValue::Unsigned(u64::MAX)
    );
    assert!(from_engineering(ValueType::U64, "18446744073709551616", "1").is_err());
    assert!(from_engineering(ValueType::I16, "32768", "1").is_err());
    assert!(from_engineering(ValueType::U16, "-1", "1").is_err());
    assert_eq!(
        from_engineering(ValueType::I64, "-9223372036854775808", "1").unwrap(),
        RawValue::Signed(i64::MIN)
    );
}

#[test]
fn known_register_vectors_cover_every_order() {
    let raw = RawValue::Unsigned(0x12345678);
    for (bytes, words, expected) in [
        (Order::Big, Order::Big, vec![0x1234, 0x5678]),
        (Order::Big, Order::Little, vec![0x5678, 0x1234]),
        (Order::Little, Order::Big, vec![0x3412, 0x7856]),
        (Order::Little, Order::Little, vec![0x7856, 0x3412]),
    ] {
        assert_eq!(
            encode(ValueType::U32, &raw, bytes, words).unwrap(),
            expected
        );
    }
    assert_eq!(
        encode(
            ValueType::U64,
            &RawValue::Unsigned(0x1122334455667788),
            Order::Big,
            Order::Little
        )
        .unwrap(),
        vec![0x7788, 0x5566, 0x3344, 0x1122]
    );
    assert_eq!(
        encode(
            ValueType::F32,
            &RawValue::Float(123456.),
            Order::Big,
            Order::Big
        )
        .unwrap(),
        vec![0x47f1, 0x2000]
    );
    assert_eq!(
        encode(
            ValueType::I16,
            &RawValue::Signed(-1),
            Order::Big,
            Order::Big
        )
        .unwrap(),
        vec![0xffff]
    );
}

#[test]
fn float_precision_and_non_finite_values_are_explicit() {
    let raw = from_engineering(ValueType::F32, "0.1", "1").unwrap();
    assert_eq!(raw, RawValue::Float(f64::from(0.1_f32)));
    assert!(from_engineering(ValueType::F32, "1e40", "1").is_err());
    assert!(from_engineering(ValueType::F64, "NaN", "1").is_err());
    assert!(from_engineering(ValueType::Bool, "true", "0.1").is_err());
}

#[test]
fn migration_requires_exact_range_and_deliberate_float_widening() {
    assert_eq!(
        convert_compatible(ValueType::U16, ValueType::I16, &RawValue::Unsigned(100)),
        Some(RawValue::Signed(100))
    );
    assert!(
        convert_compatible(ValueType::U16, ValueType::I16, &RawValue::Unsigned(50000)).is_none()
    );
    assert!(convert_compatible(ValueType::F64, ValueType::F32, &RawValue::Float(1.)).is_none());
    assert_eq!(
        convert_compatible(ValueType::F32, ValueType::F64, &RawValue::Float(1.)),
        Some(RawValue::Float(1.))
    );
}
