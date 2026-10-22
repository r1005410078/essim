//! 基础设施适配器：会话配置、真实协议运行与有界批量调度组件。
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

/// 本机串口设备发现。
pub mod serial_discovery;

/// 本机接口发现。
pub mod local_interfaces;
/// 点位分组创建适配器。
pub mod partition;

/// 原子资源预留与预算。
pub mod resources;

/// 固定并发批量任务执行资源。
pub mod batch_runner;

/// 跨调用与回调共享的端点操作门。
pub mod operation_gates;

/// 真实批量生命周期会话服务。
#[cfg(feature = "tcp-runtime")]
pub mod batch_lifecycle;

/// 有界可靠录制执行资源。
pub mod recording;

/// 实例覆盖预览与有效点表读取。
pub mod unit_overrides;
