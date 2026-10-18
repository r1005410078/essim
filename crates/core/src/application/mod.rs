//! 进程内CQRS用例，显式区分写入命令、读取查询和提交事实。
/// 跨聚合会话目录与引用协调。
pub mod catalog;
/// 命令定义及按用例族拆分的处理器。
pub mod commands;
/// 已提交配置事实及快照信封。
pub mod events;
/// 显式同步配置事件消费者。
pub mod event_handlers;
/// 仓储和资源释放窄端口。
pub mod ports;
/// 只读查询、投影与应用规划。
pub mod queries;
/// 配置事务及释放协调。
pub mod service;
/// 运行、录制与回放契约。
pub mod runtime;
/// 请求停止与释放确认的窄适配。
pub mod resource_release;
/// 旧配置导入重导出门面。
pub mod workspace;
