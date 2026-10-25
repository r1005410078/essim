use lib60870_rs::{Asdu, Cause, Point, Quality};
#[test]
fn independent_units_roundtrip_and_mixed_types_are_rejected_without_append() {
    let mut asdu = Asdu::new(47, Cause::Spontaneous, false).unwrap();
    let p = Point::single(100, true, Quality::GOOD).unwrap();
    asdu.add(p).unwrap();
    assert_eq!(asdu.common_address(), 47);
    assert_eq!(asdu.type_id(), 1);
    assert_eq!(asdu.point(0).unwrap(), p);
    assert!(asdu
        .add(Point::measured(101, 12.5, Quality::GOOD).unwrap())
        .is_err());
    assert_eq!(asdu.len(), 1);
    assert!(asdu.point(1).is_err());
    let mut other = Asdu::new(48, Cause::Periodic, false).unwrap();
    other
        .add(Point::single(100, false, Quality::GOOD).unwrap())
        .unwrap();
    assert_ne!(asdu.point(0).unwrap(), other.point(0).unwrap());
}
#[test]
fn address_quality_sequence_and_capacity_are_validated() {
    assert!(Point::single(0x1000000, true, Quality::GOOD).is_err());
    assert!(Quality::new(2).is_err());
    assert!(Point::single(1, true, Quality::new(1).unwrap()).is_err());
    assert!(Point::measured(1, f32::NAN, Quality::GOOD).is_err());
    let mut a = Asdu::new(1, Cause::Periodic, true).unwrap();
    a.add(Point::measured(100, 1.5, Quality::GOOD).unwrap())
        .unwrap();
    assert!(a
        .add(Point::measured(102, 2.5, Quality::GOOD).unwrap())
        .is_err());
    assert_eq!(a.len(), 1);
    a.add(Point::measured(101, 2.5, Quality::GOOD).unwrap())
        .unwrap();
    assert_eq!(
        a.point(1).unwrap(),
        Point::measured(101, 2.5, Quality::GOOD).unwrap()
    );
    while a
        .add(Point::measured(100 + a.len() as u32, 0., Quality::GOOD).unwrap())
        .is_ok()
    {}
    assert!(a.len() < 127);
    let n = a.len();
    assert!(a
        .add(Point::measured(100 + n as u32, 0., Quality::GOOD).unwrap())
        .is_err());
    assert_eq!(a.len(), n);
}
