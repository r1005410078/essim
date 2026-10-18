# ESSim

面向真实 EMS 联调、储能设备协议学习和可复现实验的 Rust + Tauri + React 工作台。

[项目目标](docs/project-goals.md) · [Modbus实现计划](docs/plans/modbus-simulator.md) · [架构](docs/architecture/workspace.md) · [流程图与代码对应](docs/design/modbus-event-storming.md) · [开发约束](AGENTS.md)

## 当前能力

M01已完成约定的[基础模型与工作台提案](openspec/changes/establish-modbus-simulator-foundation/proposal.md)：

- 具有稳定身份的设备、采集模型、TCP／RTU主从端点及逻辑单元。单元通过稳定ID引用当前模型，可选择关联设备。
- 会话内配置目录、稳定模型引用与覆盖保存、保存与显式应用分离、全部单元校验后原子应用。
- Dockview停靠工作台，`@pierre/trees`资源／模拟器树，NovaSheet点表，属性栏Unit切换和四遥导航。
- 运行／录制失败门禁及隔离回放的核心契约与测试；尚无生产执行器、可靠录制和可用回放，相关操作明确禁用。

**当前配置仅保留本次会话，退出即丢失。** SQLite、多实例通信、采集调度及场景／拓扑／3D均未交付。默认项目为空，不放入演示设备或假运行记录。浏览器预览无后端时明确失败。

M02正在实施[点表管理提案](openspec/changes/implement-modbus-point-table-management/proposal.md)：已接入树右键CRUD、NovaSheet编辑、精确数值编码、模型引用、Markdown帮助和JSON／CSV／XLSX交换，自动化及主要原生流程已验证，下拉修改与属性同步已由用户确认，剩余快捷键／键盘帮助复核中。实际测试及限制见[实施记录](docs/test-records/2026-10-16-m02-progress.md)，未将实现中的功能标为整体验收完成。

旧固定 `demo-bms`／SOC TCP与RTU教学链路已退役，旧无界面示例和验证脚本不再提供。历史结果只证明当时实现，不能当作新工作台协议运行验收。清理映射与本轮验证见 [M01实施记录](docs/test-records/2026-10-15-m01-foundation.md)。真实运行由后续 M04/M05 接入。

## 开发

```sh
pnpm install
pnpm --filter @essim/workbench dev:web    # 浏览器预览，无Rust后端
pnpm --filter @essim/workbench dev:tauri  # 桌面工作台，自动启动Web开发服务
pnpm --filter @essim/workbench test
pnpm --filter @essim/workbench build
cargo test --workspace --offline
cargo doc --workspace --no-deps --offline
```

正式入口在 `workbench/`，共享UI在 `packages/ui/`。`prototype/`保留原有独立工具链，只表达原型业务交互，不是当前产品协议后端。学习资料见 [学习路线](docs/learning-roadmap.md)，其中教学SOC课程为历史实验。

## 通用 libmodbus 封装

`crates/libmodbus/{libmodbus-src,libmodbus-sys,libmodbus-rs}`继续独立保留：官方固定源码构建、原始绑定和Rust安全封装。提供四区Mapping、TCP／RTU从站上下文、同步TCP／RTU主站四区读取及可写区单／多写。工作台当前不链接此库；库可用不等于产品执行器已接入。

默认 vendored 静态链接；非vendored必须显式指定自行编译的动态前缀，不探测Homebrew或其他宿主库。构建方式及安全／协议边界见 [三包说明](crates/libmodbus/README.md)。

```sh
cargo test -p libmodbus-rs -p libmodbus-sys -p libmodbus-src --offline
cargo build -p libmodbus-rs --example client_demo --offline
# Python环境需要pymodbus==3.11.3、pyserial==3.5；示例会写入测试从站。
python3 scripts/verify_modbus_master.py
```

动态模式在相应命令前设置 `LIBMODBUS_DIR`，并传 `--no-default-features`。独立PyModbus TCP／PTY互操作不能替代真实EMS或真实RS-485硬件验收，也未证明Linux／Windows兼容、完整标准符合性或规模性能。

历史证据：[TCP教学闭环](docs/test-records/2026-10-14-modbus-loop.md) · [三包静态／动态](docs/test-records/2026-10-15-libmodbus-workspace.md) · [动态Unit](docs/test-records/2026-10-15-dynamic-unit-id.md) · [RTU软件闭环](docs/test-records/2026-10-15-rtu.md) · [通用主站](docs/test-records/2026-10-15-modbus-master.md)。真实RS-485任务9.9仍待验。

配置现已取消版本管理：保存直接覆盖当前模型，单元只选择模型；复制和导出使用当前已保存内容。保存不自动应用，已应用数据使用独立快照。[简化记录](docs/test-records/2026-10-16-remove-config-versions.md)。

M03模拟器配置已接入：默认Unit与模型、固定类型、复制／删除、文件夹、草稿关闭保护、核心应用差异和停止态Unit复位。39／41项任务完成，原生快捷键及编辑中保存等验收仍待收尾；详见[实施记录](docs/test-records/2026-10-17-m03-configuration.md)。真实协议运行、可靠录制和SQLite尚未接入，保存仅限本次会话。


配置架构已按CQRS／DDD职责拆分：[命令与处理器](crates/core/src/application/commands/mod.rs)、[查询／投影](crates/core/src/application/queries/mod.rs)、[具体配置事件](crates/core/src/application/events/mod.rs)、[聚合目录](crates/core/src/domain/aggregates/mod.rs)、[同步事件监听器](crates/core/src/application/event_handlers/mod.rs)。成功提交后返回具体事实，不将快照通知称为事件溯源。[重构提案](openspec/changes/refactor-workspace-cqrs-boundaries/proposal.md) · [验证记录](docs/test-records/2026-10-17-cqrs-boundaries.md)。
