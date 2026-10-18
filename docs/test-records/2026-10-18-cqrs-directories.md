# CQRS目录细分验证（2026-10-18）

## 范围

在既有CQRS重构上继续按职责拆文件：events区分配置事实、信封及运行历史契约；queries区分查询结果、能力、投影状态、应用预览和差异检查；domain/aggregates按业务边界组织，端点和模型进一步分离聚合根及所属类型。原领域模块和runtime历史类型导入保持兼容。

服务提交后显式同步调用event_handlers中的WorkspaceProjectionListener。原Projection::accept委托同一消费者；消费整个配置信封更新快照，不按具体fact动态注册或路由。没有新增数据库、异步总线、协议执行器或可靠投递承诺。

## 验证

- 修改前：`cargo test -p essim-core -p essim-infrastructure --quiet`通过，71个集成测试及1个文档测试。
- 修改后：同一命令通过，72个集成测试及1个文档测试。新增投影监听器回归覆盖快照更新、重复幂等、序号缺口、未知schema及拒绝后保持原状态。既有提交事实、失败原子性、资源释放并发测试通过。
- 架构边界检查改为递归扫描，覆盖新增聚合目录及所有查询文件。
- `cargo doc -p essim-core -p essim-infrastructure --no-deps --quiet`通过。
- `cargo check -p essim-workbench --quiet`通过，确认桌面适配层导入兼容。
- `openspec validate refactor-workspace-cqrs-boundaries --strict`通过；`git diff --check`通过，更新文档的本地链接目标检查通过。

过程中一次测试进程报告可执行文件不存在而未能启动；当时存在重叠构建，具体原因未单独确认。随后完整重跑通过，没有把该次未运行记为通过。

本轮没有修改前端逻辑或通信协议，未重跑前端测试、原生交互及互操作验收。M02／M03遗留验收保持原状态。

[分层目录](../architecture/workspace.md#文件分层2026-10-18) · [实际调用时序](../design/modbus-event-storming.md#0-当前代码新建模拟器的命令查询与通知) · [实施任务](../../openspec/changes/refactor-workspace-cqrs-boundaries/tasks.md)
