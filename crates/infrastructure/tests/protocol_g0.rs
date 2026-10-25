//! 仅供G0的候选契约设施，不被lib.rs、工作台、生产注册表引用。
//! 使用真实OperationGates／RecordingPool；共享状态和61850模型是受限反例设施。
#[path = "protocol_g0/control.rs"]
mod control;
#[path = "protocol_g0/models.rs"]
mod models;
#[path = "protocol_g0/state.rs"]
mod state;
