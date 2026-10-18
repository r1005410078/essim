//! M01边界类型；核心已定义稳定字符串ID及精确的十进制数值编码。
// DTO直接复用无框架依赖的序列化值类型，避免复制字段造成契约漂移。
pub use essim_core::application::{
    commands::{Command, Receipt},
    queries::WorkspaceView,
};
pub use essim_core::domain::error::Error as ErrorDto;
