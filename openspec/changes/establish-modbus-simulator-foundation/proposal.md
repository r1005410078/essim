> 2026-10-16 后续范围调整：本文原有模型／配置版本规则已由M02单份当前配置与已应用快照方案取代，历史完成记录保持原边界。见[当前规格](../implement-modbus-point-table-management/specs/point-table-catalog/spec.md)。

## Why

现有 core、基础设施、IPC 和界面围绕固定 `demo-bms`、SOC 及地址100的教学闭环构建，无法清楚表达可选设备关联、共享端点、独立单元、模型版本和运行历史。M01 已确认基础边界及工作台方向，需要先形成不依赖教学业务的正式骨架，再逐项实施 M02–M10。

## What Changes

- 建立具有稳定 ID 的设备、采集模型、通信配置、逻辑单元、运行会话和回放会话模型；设备关联可选，角色属于端点／连接／通道。
- 明确采集模型、端点／通道配置两个聚合的一致性边界；一张点表包含公共映射与角色相关配置，四遥分类与 Modbus 四区分开。
- 采用进程内 CQRS：配置命令、只读查询、执行器和录制端口分离；定义应用版本、运行确认、事件顺序和独立回放的契约及测试，不引入分布式消息平台。
- 固化一次端点／通道运行对应一条记录、录制失败停止对应运行、配置版本显式应用、会话内运行值保留／迁移等规则。M01 用可控测试适配器验证契约，不将内存记录当作可靠落盘。
- 依据 [v15 概念稿](../../../docs/design/m01-workbench-v15.png) 建立浅色紧凑 Dockview 工作台骨架：端点主 Tab、纯图标工具栏、属性栏 Unit 切换、UE5 Details 式属性结构及底部四遥紧凑导航。未交付操作明确禁用或标为未接入。
- 设备／资源与模拟器导航树使用 `@pierre/trees`，点表继续复用已接入的 NovaSheet；树路径通过适配层关联稳定业务 ID，重命名或移动不改变对象身份。
- **BREAKING**：在实施本提案时移除／替换正式工程的固定 SOC 服务、教学 worker、旧 IPC 和单点面板，不保留旧教学调用兼容。保留 libmodbus 三包、通用测试、原型、构建工具及历史验收材料；逐文件清单见 design.md。

## Capabilities

### New Capabilities

- `modbus-foundation-model`: 稳定身份、采集模型与通信聚合、单元引用、版本应用和运行值规则。
- `simulation-cqrs-contracts`: 命令／查询、运行所有权、执行确认、完整录制与隔离回放契约。
- `modbus-workbench-foundation`: v15 工作台骨架、对象选择、纯图标操作和真实能力边界。
- `teaching-loop-retirement`: 教学接入清理、保留清单及清理后构建／通用验证基线。

### Modified Capabilities

无。当前无已归档主规格；旧接入与封装变更及其验收保留为历史，不在本提案中改写或自动归档。

## Impact

实施涉及 `crates/core`、`crates/infrastructure` 的业务适配、`workbench` 的 IPC／统一客户端及界面组装、教学专用测试与脚本、README 和架构文档。不修改第三方 vendored 源码，不删除通用 libmodbus 包或 NovaSheet 快照，不重建协议栈，不更换 Rust/Tauri/React 技术栈。

实施结果见[M01验收记录](../../../docs/test-records/2026-10-15-m01-foundation.md)，任务勾选与实际证据同步。M01 完成意味着基础模型、契约、清理和可启动工作台骨架通过验证，不意味着设计稿中的全部运行能力完成。完整 CRUD／编码转换、真实多实例协议运行、采集调度、SQLite及可靠运行历史存储、可用离线回放、场景／拓扑／3D和动态仿真分别由 M02–M10 提案交付；真实 RS-485 硬件验证继续待验。
