# 配置 CQRS／DDD 职责拆分记录（2026-10-17）

关联[提案](../../openspec/changes/refactor-workspace-cqrs-boundaries/proposal.md)、[设计](../../openspec/changes/refactor-workspace-cqrs-boundaries/design.md)、[任务](../../openspec/changes/refactor-workspace-cqrs-boundaries/tasks.md)。本轮按用户要求先形成提案，再实施拆分；不自动提交或归档。

## 基线与范围

开工时工作区已有M01／M02／M03、移除配置版本及点表／保存修复等大量未提交改动，本轮保留。开工核心／基础设施62项集成测试及1项文档测试通过。M03保持39／41，6.5及8.4仍未完成；M02保持32／33。本轮不替代这些原生验收。

修改范围为core领域模块和配置应用层、基础设施及Tauri的Rust导入与事件入口、新增架构／事实回归和文档。不改前端实现、协议库、包依赖、Turborepo或点表交换格式。

## 已实施的结构

| 职责 | 入口 |
| --- | --- |
| 写入意图和确认 | `application/commands/mod.rs`的Command、Action、Receipt |
| 配置处理 | `commands/model.rs`、`endpoint.rs`、`navigation.rs`、`device.rs` |
| 应用／删除／复位处理 | `commands/lifecycle.rs`，通过窄上下文使用资源端口与停止状态 |
| 原子提交和分段释放协调 | `application/service.rs`的Workspace／ReleasePlan |
| 查询、投影、纯迁移预览及草稿比较 | `application/queries.rs` |
| 具体配置事实及提交信封 | `application/events.rs`的ConfigurationFact／ConfigEvent |
| 仓储与停止端口 | `application/ports.rs` |
| 跨聚合目录、引用检查 | `application/catalog.rs` |
| 领域规则 | `domain/identity.rs`、`error.rs`、`model.rs`、`endpoint.rs`、`device.rs`、`navigation.rs`、`values.rs`、`serialization.rs`及原有`point_table.rs` |
| 基础设施事件入口 | `WorkspaceEngine::execute_with_event`，原execute继续只返回Receipt |

原`application/workspace.rs`及`domain/foundation.rs`分别成为8行与6行的重导出门面，没有第二套处理逻辑。仓内生产调用已迁到明确模块；部分既有测试继续使用旧入口，也覆盖导入兼容性。

端点类型固定由`EndpointConfig::ensure_same_type`检查；文件夹保存、循环、移动目标和非空删除归`Navigation`方法。跨模型／设备引用及全局Unit身份检查仍属于应用协调。Catalog是会话存储边界，没有把所有资源谎称为同一个领域聚合。

生命周期处理器不直接修改查询投影：发现外部权威变化时返回观测到的目录，由服务更新只读投影并返回失败。原有“规划后放锁等待，确认后重新核对目标与依赖，保留无关并发保存”的路径保持不变。

## 事件和事务语义

成功配置操作提供13种具体事实：ModelSaved／Copied／Deleted、EndpointCreated／Saved／Copied／Deleted／Applied／Moved、UnitValuesReset、FolderSaved／Deleted和DeviceCreated。复制事实携带源和新身份，复位携带端点和Unit，移动携带父目录。

处理器只准备事务内结果，服务在仓储commit成功后才返回ConfigEvent并推进配置通知序号。校验、停止或提交失败只有错误结果；不会返回成功事件。实际停止状态独立保留，不因配置提交失败恢复成运行中。

ConfigEvent的fact表达业务含义，catalog保留现有查询恢复快照；二者不是同一个概念。现有前端IPC仍为Command／Receipt／WorkspaceView，内部新增fact没有改前端Action JSON或点表文件格式。配置事实没有事件总线、持久化、可靠投递或完整事件回放能力；运行Fact／HistoryEvent仍是现有契约，真实可靠录制归后续阶段。不增加配置版本或expectedRevision。

## 测试与结果

先增加`configuration_events.rs`用例，确认旧实现缺少fact而失败；另写直接领域调用用例，确认旧类型尚未提供局部不变量方法，再实施规则迁移。

- `cargo test -p essim-core -p essim-infrastructure --offline`：71项集成测试及1项文档测试通过。保留全部原62项，新增4项配置事实／失败／查询纯度、2项直接领域不变量、2项依赖与门面边界、1项适配器带事件入口测试。
- `pnpm --filter @essim/workbench test`：44项通过。
- `pnpm --filter @essim/workbench build`：类型检查和生产构建通过，保留原有大chunk提示。
- `cargo build -p essim-workbench --offline`及`cargo doc -p essim-core -p essim-infrastructure --no-deps --offline`：通过。
- `cargo test --workspace --offline`：未通过；libmodbus-sys构建仍缺少autoconf，未修改协议库或安装工具规避限制。
- OpenSpec严格校验与本地文档链接、`git diff --check`检查通过。

| 规格场景 | 验证 |
| --- | --- |
| 命令／事件／查询可定位，查询无副作用 | 模块入口及`architecture_boundaries.rs`、`configuration_events.rs`查询预览／inspection用例 |
| 直接领域类型拒绝非法类型修改和目录循环 | `domain_boundaries.rs`；原`simulator_configuration.rs`和`foundation.rs`保持通过 |
| 成功提交事实的源／目标／Unit及快照一致 | 核心`configuration_events.rs`覆盖13种事实，基础设施同名测试验证只提交一次 |
| 校验／释放／存储失败无成功事件或通知序号推进 | 核心配置事件失败用例，原M03失败仓储及释放并发用例 |
| 释放等待期间无关保存继续、目标改变拒绝旧操作 | 原`infrastructure/tests/simulator_release.rs`保持通过 |
| 既有前端及协议／历史边界保持 | 前端44项、桌面构建、原编码／迁移／运行契约测试；M02／M03未验项未勾选 |

本轮仅重构和配置事实增量，没有重做原生UI流程，也没有新增真实Modbus或规模证据。M03原生快捷键／编辑中保存及最后inspection复测继续按原任务跟踪。
