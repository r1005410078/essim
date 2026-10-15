// 领域层不依赖 Tauri、数据库或协议库。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendInfo {
    pub name: String,
    pub version: String,
    pub protocol_engine_ready: bool,
}
pub mod modbus;
