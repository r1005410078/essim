use essim_core::domain::modbus::{Endpoint, Lifecycle, Soc, State};
#[test]
fn soc_validation_and_initial_value_are_separate() {
    for value in [-1.0, 1001.0, 650.5, f64::NAN] {
        assert!(Soc::new(value).is_err());
    }
    assert_eq!(Soc::new(700.0).unwrap().engineering(), 70.0);
    assert_eq!(Soc::INITIAL.raw(), 650);
}
#[test]
fn lifecycle_rejects_duplicate_start_and_stop_is_idempotent() {
    let mut lifecycle = Lifecycle::default();
    lifecycle.starting().unwrap();
    lifecycle.listening();
    assert!(lifecycle.starting().is_err());
    lifecycle.stopped();
    lifecycle.stopped();
    assert_eq!(lifecycle.state(), State::Stopped);
    lifecycle.failed();
    assert!(lifecycle.starting().is_ok());
}
#[test]
fn endpoint_rejects_invalid_addresses_and_ports() {
    assert!(Endpoint::new("hostname", 1502).is_err());
    assert!(Endpoint::new("127.0.0.1", 0).is_err());
}
