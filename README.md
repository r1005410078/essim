# ESSim

面向真实 EMS 联调、储能设备协议学习和可复现实验的 Rust + Tauri + React 工作台。

[项目目标](docs/project-goals.md) · [Modbus实现计划](docs/plans/modbus-simulator.md) · [架构](docs/architecture/workspace.md) · [流程图与代码对应](docs/design/modbus-event-storming.md) · [开发约束](AGENTS.md)

## 当前能力

M01已完成约定的[基础模型与工作台提案](openspec/changes/establish-modbus-simulator-foundation/proposal.md)：

- 具有稳定身份的设备、采集模型、TCP／RTU主从端点及逻辑单元。单元通过稳定ID引用当前模型，可选择关联设备。
- 会话内配置目录、稳定模型引用与覆盖保存、保存与显式应用分离、全部单元校验后原子应用。
- Dockview停靠工作台，`@pierre/trees`资源／模拟器树，NovaSheet点表，属性栏Unit切换和四遥导航。
- 运行／录制失败门禁及隔离回放的核心契约与测试；当时尚无生产执行器；当前 TCP 从站及文件录制进展见下文，可用回放仍未接入。

**当前配置仅保留本次会话，退出即丢失。** SQLite及场景／拓扑／3D均未交付；TCP 多端点运行已接入，规模边界见下文。默认项目为空，不放入演示设备或假运行记录。浏览器预览无后端时明确失败。

M02正在实施[点表管理提案](openspec/changes/implement-modbus-point-table-management/proposal.md)：已接入树右键CRUD、NovaSheet编辑、精确数值编码、模型引用、Markdown帮助和JSON／CSV／XLSX交换，自动化及主要原生流程已验证，下拉修改与属性同步已由用户确认，剩余快捷键／键盘帮助复核中。实际测试及限制见[实施记录](docs/test-records/2026-10-16-m02-progress.md)，未将实现中的功能标为整体验收完成。

旧固定 `demo-bms`／SOC TCP与RTU教学链路已退役，旧无界面示例和验证脚本不再提供。历史结果只证明当时实现，不能当作新工作台协议运行验收。清理映射与本轮验证见 [M01实施记录](docs/test-records/2026-10-15-m01-foundation.md)。真实运行由后续 M04/M05 接入。

## 开发

连接工作台TCP从站的Python示例见 [example](example/README.md)，默认只读保持寄存器，不修改运行值。

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

`crates/libmodbus/{libmodbus-src,libmodbus-sys,libmodbus-rs}`继续独立保留：官方固定源码构建、原始绑定和Rust安全封装。提供四区Mapping、TCP／RTU从站上下文、同步TCP／RTU主站四区读取及可写区单／多写。工作台 TCP 从站执行器已链接此库；TCP主站工作台本轮接入，RTU工作台运行仍待接入。

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

M03模拟器配置已接入：默认Unit与模型、固定类型、复制／删除、文件夹、草稿关闭保护、核心应用差异和停止态Unit复位。39／41项任务完成，原生快捷键及编辑中保存等验收仍待收尾；详见[实施记录](docs/test-records/2026-10-17-m03-configuration.md)。M03阶段的保存仅限本次会话；后续TCP真实运行及文件录制进展见下文，SQLite仍未接入。


配置架构已按CQRS／DDD职责拆分：[命令与处理器](crates/core/src/application/commands/mod.rs)、[查询／投影](crates/core/src/application/queries/mod.rs)、[具体配置事件](crates/core/src/application/events/mod.rs)、[聚合目录](crates/core/src/domain/aggregates/mod.rs)、[同步事件监听器](crates/core/src/application/event_handlers/mod.rs)。成功提交后返回具体事实，不将快照通知称为事件溯源。[重构提案](openspec/changes/refactor-workspace-cqrs-boundaries/proposal.md) · [验证记录](docs/test-records/2026-10-17-cqrs-boundaries.md)。


M04 TCP从站已接入真实监听、四区基础8功能码、多Unit／多客户端、原子置数／复位、启停保值和可靠文件录制。工作台运行栏通过独立运行命令／查询访问固定线程所有者；RTU诊断、配置数据库、历史查看及可用回放仍待实施。原生启停、置数、复位及独立主站读取已验证；1000端点典型亿点负载尚未完成验收，见[M04实施记录](docs/test-records/2026-10-18-m04-tcp-slave.md)及[提案任务](openspec/changes/implement-modbus-tcp-slave-runtime/tasks.md)。

无界面入口读取已应用快照数组JSON（Snapshot字段为endpoint及values），不依赖Tauri窗口：

```bash
cargo run -p essim-infrastructure --features tcp-runtime,vendored --example tcp_slave -- /absolute/path/snapshots.json /absolute/path/run-records
# stdin输入quit后确认关闭全部监听与记录；测试快照及独立主站验证见下述脚本。
python3 scripts/verify_tcp_slave_runtime.py --binary target/debug/examples/tcp_slave
```

独立验证脚本需PyModbus 3.11.3。运行记录目录与配置保存不是同一功能；JSONL格式为当前内部运行记录，不宣称已提供稳定的项目交换格式。

运行表刷新跟随当前可见行，与选区无关；编辑草稿后点击“提交修改（数量）”提交当前Unit各可编辑分类的全部改动。外部刷新保留未提交草稿，恢复初值通过指定数据区地址范围或整Unit复位。[交互验收](docs/test-records/2026-10-18-m04-tcp-slave.md#运行表交互修订)。

工作台外围已按确认方案整理：项目树、配置／运行操作、上下文属性、应用迁移预览和真实通信／事件／问题面板。点表保留 NovaSheet 样式；报文可筛选、暂停展示和导出当前视图（有界最近窗口，不是完整抓包）。见[UI 实施与原生截图](docs/test-records/2026-10-18-workbench-ui-v2.md)。M07 整体、历史查看与回放仍未完成。

通信报文字节现支持点击／Enter打开字段解析，覆盖基础8功能码的TCP请求、响应及异常；读响应仅在唯一匹配请求时补地址。“详细”进入Dockview中的[Modbus指南](workbench/src/resources/protocols/modbus/guide.md)，项目树“文档资源”与采集模型平级，按协议分类、离线Markdown渲染。见[本轮记录](docs/test-records/2026-10-18-modbus-reference.md)与[解析边界](docs/test-records/2026-10-18-message-hover.md)。


M05 TCP主站：单目标共享连接、多Unit独立结果、四区手动／周期读取、遥控／遥调显式下发及回验、质量／更新时间、采集中自动重连和真实字节观测。默认连接／请求／重连为3000／1000／3000ms，过期倍数3；主站设置进入配置草稿，保存并应用后生效。应用会断开并保持断开；待写草稿需明确丢弃。首版最多32个主站所有者槽，单批手动写入最多1024点（先整批校验，再按功能码上限拆请求），非主站规模性能承诺。RTU主站另立提案；[实施与验收边界](docs/test-records/2026-10-19-m05-tcp-master.md)。

```sh
cargo build -p essim-infrastructure --features tcp-runtime,vendored --example tcp_master --offline
# 需要PyModbus 3.11.3；只访问脚本自己启动的回环从站。
python3 scripts/verify_tcp_master_runtime.py
```
