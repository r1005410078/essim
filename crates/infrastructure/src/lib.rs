//! 基础设施适配器。M01仅装配会话内配置，不提供协议运行或可靠录制。
/// 会话内配置目录与应用入口。
pub mod workspace;
/// 点表输入预览与文件格式交换。
pub mod point_exchange;

/// 正式TCP从站运行适配器。
#[cfg(feature="tcp-runtime")]
pub mod tcp_slave;

/// 正式TCP主站运行适配器。
#[cfg(feature="tcp-runtime")]
pub mod tcp_master;
