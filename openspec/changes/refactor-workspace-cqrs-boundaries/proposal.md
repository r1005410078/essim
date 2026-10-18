## Why

当前配置命令、查询投影、事件、仓储端口和业务处理集中在一千余行的`application/workspace.rs`，领域类型集中在`foundation.rs`，读者难以辨认CQRS和DDD职责。需要在M03现有行为基础上拆清代码，并让配置成功后的具体业务事实可见，避免将目录快照通知误称为完整领域事件架构。

## What Changes

- 将配置应用层拆为命令定义及按模型／端点／导航／生命周期组织的处理器、只读查询与投影、事件、端口及薄用例协调服务。
- 将身份、错误、模型、端点、值迁移和导航拆为领域模块；端点类型不变、文件夹循环等局部不变量归对应领域类型，跨资源引用及事务协调保留应用层。
- 为已提交配置增加强类型业务事实，例如EndpointCreated、EndpointDeleted、EndpointApplied和UnitValuesReset。提交失败不返回成功事件；保留现有完整快照用于会话查询，明确两者职责。
- 保持前端命令JSON、Tauri入口、保存／应用边界、稳定ID、原子性及释放等待期间并发行为；旧Rust导入入口仅作为无逻辑重导出门面，仓内正式调用迁到明确模块。
- 不引入事件总线、消息中间件、事件溯源、配置版本、SQLite、生产协议执行器或新的前端功能。M02／M03未完成验收独立保留。

2026-10-18补充分层：events、queries按职责拆成目录；聚合根及所属类型归入domain/aggregates；既有同步投影消费独立到event_handlers，不增加动态事件总线。

## Capabilities

### New Capabilities

- `workspace-cqrs-boundaries`: 清楚可定位的命令处理、只读查询、提交事实和领域不变量边界，以及行为保持的架构回归。

### Modified Capabilities

无已归档主规格；本轮新增架构约束，不修改M01／M02／M03产品规则。

## Impact

涉及`crates/core/src/application`、`domain`、基础设施和Tauri的Rust导入、核心及适配器测试、架构文档。仅内部配置事件增加业务事实字段，前端现有IPC与点表文件格式不变。不改协议库、前端组件、包依赖或Turborepo。重构及事件增量均以现有测试为基线验证，不将规划完成当作产品交付。
