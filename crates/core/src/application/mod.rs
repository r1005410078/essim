use crate::domain::BackendInfo;
pub fn backend_info() -> BackendInfo {
    BackendInfo {
        name: "ESSim".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        protocol_engine_ready: false,
    }
}
pub mod simulator;
