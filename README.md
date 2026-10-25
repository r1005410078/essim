# ESSim

面向真实 EMS 联调、储能设备协议学习和可复现实验的 Rust + Tauri + React 工作台。

[项目目标](docs/project-goals.md) · [Modbus实现计划](docs/plans/modbus-simulator.md) · [架构](docs/architecture/workspace.md) · [流程图与代码对应](docs/design/modbus-event-storming.md) · [开发约束](AGENTS.md)

新增协议先参考[协议接入规范与 Modbus 接口审查](docs/design/protocol-integration.md)，以及[面向104／61850的深度审查](docs/reviews/2026-10-25-protocol-integration.md)：包含当前接口、设备状态／协议模型边界和集成风险；建议公共接口仍是草案，尚未改造生产运行路径。

已形成[协议接入与通信管理重构提案](openspec/changes/refactor-protocol-integration-and-communications/proposal.md)：[G0验证](docs/test-records/2026-10-25-protocol-g0.md)已补齐前四项及第二轮两项复核遗漏，并以30项用例重新确认受限接口／故障及104真实TCP队列策略（含Gap后旧快照拒绝与记录队列拒绝后恢复）。后续通信管理兼容、共享状态／原子应用、工程格式／展示尚未迁移；不表示104／61850工作台已支持。

顶部已接入[资源／命令检索](docs/design/command-center.md)、文档历史前进后退与面板开关；Tabs仍在中央内容区，拼音索引按需加载。浏览器交互已检查，macOS已检查原生检索、焦点恢复及缩放；最小化恢复、拖动位置与退出保护待补验，Windows/Linux尚未验收。[提案](openspec/changes/implement-workbench-command-center/proposal.md) · [实施记录](docs/test-records/2026-10-23-command-center-ui.md)。

## 当前能力

M01已完成约定的[基础模型与工作台提案](openspec/changes/establish-modbus-simulator-foundation/proposal.md)：

- 具有稳定身份的设备、采集模型、TCP／RTU主从端点及逻辑单元。单元通过稳定ID引用当前模型，可选择关联设备。
- 会话内配置目录、稳定模型引用与覆盖保存、保存与显式应用分离、全部单元校验后原子应用。
- Dockview停靠工作台，`@pierre/trees`资源／模拟器树，NovaSheet点表，属性栏Unit切换和四遥导航。
- 运行／录制失败门禁及隔离回放的核心契约与测试；当时尚无生产执行器；当前 TCP 从站及文件录制进展见下文，可用回放仍未接入。

左侧导航采用亮色图标标签：服务器图标切换模拟器，叠层图标切换资源（采集模型、设备和文档）。右侧“＋”按当前分类新建，“更多”提供批量任务和运行资源；文件夹及业务图标按类别配色。[本轮验证](docs/test-records/2026-10-22-navigation-tabs.md)。

Unit 右键菜单按单元身份提供新增、重命名和删除；模拟器节点保留整机操作。单元修改即时反映到树，保留在配置草稿中，保存后需显式应用。菜单统一使用图标并移除文字末尾省略号。关闭弹框后按稳定身份恢复树焦点，macOS 已复核连续方向键及 Shift+F10。[修复与验证](docs/test-records/2026-10-24-unit-context-menu.md)。

树节点最右侧现显示彩色运行状态和独立配置图标，hover查看说明；状态来自现有真实运行查询，不从端点监听推导Unit在线。见[设计规范](docs/design/tree-status.md)、[提案](openspec/changes/show-resource-tree-status/proposal.md)及[原生验证](docs/test-records/2026-10-24-tree-status.md)。

事件记录／通信报文／问题现位于中央工作区下方的 Dockview 圆角面板，宽度与文档区对齐，默认展开并选中通信报文；状态栏压缩为22px，显示连接、端点、待应用和问题计数。[验证记录](docs/test-records/2026-10-22-bottom-edge.md)。

首开窗口目标为1280×800逻辑像素，按显示器可用区域缩小并居中；左右栏与底部按初始视口分配比例。[布局规则](docs/design/startup-layout.md) · [原生验证](docs/test-records/2026-10-22-startup-layout.md)。

欢迎页、模拟器空态与未选对象的属性栏采用轻量工程线稿，以细灰轮廓、少量浅色和留白表达EMS联调、设备机柜与点表检查；底部事件／通信空态使用简洁通信符号，问题空态复用点表检查线稿。[插画与界面验证](docs/test-records/2026-10-23-light-illustrations.md)。

类型、字节序与字序下拉直接显示原始标识（如 `f64`、`big`、`little`），保留 Markdown 帮助。底层保留协议标识；模型属性栏与配置表共用选项。[验证记录](docs/test-records/2026-10-23-point-select-options.md)。

字段帮助与点表枚举说明统一复用公共 [MarkdownTooltip](docs/design/markdown-tooltip.md)，使用与提示框同色的原生三角箭头。

点表数据区下拉显示中文名称、英文标识和位宽／访问属性，选中后单元格仅显示中文；悬停或键盘聚焦选项可查看 Markdown 协议说明。[验证记录](docs/test-records/2026-10-23-modbus-area-dropdown.md)。

**已接入工程持久化：静态资源文件、运行 SQLite、最近工程与草稿恢复；正式桌面必须打开或新建工程后才能进入工作台。** 场景／拓扑／3D未交付；TCP 多端点运行已接入，规模边界见下文。默认项目为空，不放入演示设备或假运行记录。浏览器预览无后端时明确失败。

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

业务弹框已统一为工程紧凑公共组件，支持标题栏拖动、八方向缩放、复位和固定操作栏。后续接入见[弹框规范](docs/design/workbench-dialog.md)；[本轮验证](docs/test-records/2026-10-22-compact-dialogs.md)。

正式入口在 `workbench/`，共享UI在 `packages/ui/`。`prototype/`保留原有独立工具链，只表达原型业务交互，不是当前产品协议后端。学习资料见 [学习路线](docs/learning-roadmap.md)，其中教学SOC课程为历史实验。

属性面板已统一接入共享组件，采用紧凑分类属性表；后续字段、协议与对象按[接入规范](docs/design/property-panel.md)扩展。

点表配置页采用单行图标工具栏与可搜索Unit切换，表格恢复NovaSheet默认25px行高／表头，移除顶部字段说明。入口与扩展方式见[点表工具栏规范](docs/design/point-toolbar.md)。运行视图同样合并为38px单行图标工具栏，未应用说明进入状态提示，真实错误／未确认结果保留显式警报；[验证记录](docs/test-records/2026-10-22-runtime-toolbar.md)。

## 通用 libmodbus 封装

`crates/libmodbus/{libmodbus-src,libmodbus-sys,libmodbus-rs}`继续独立保留：官方固定源码构建、原始绑定和Rust安全封装。提供四区Mapping、TCP／RTU从站上下文、同步TCP／RTU主站四区读取及可写区单／多写。工作台 TCP 从站执行器已链接此库；TCP主站工作台本轮接入，RTU主站软件已接入，RTU从站工作台运行仍待接入。

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

M03模拟器配置已接入：默认Unit与模型、固定类型、复制／删除、文件夹、草稿关闭保护、核心应用差异和停止态Unit复位。39／41项任务完成，原生快捷键及编辑中保存等验收仍待收尾；详见[实施记录](docs/test-records/2026-10-17-m03-configuration.md)。M03阶段的保存仅限本次会话；后续TCP真实运行及文件录制进展见下文，该阶段尚未接入 SQLite；当前工程存储进展见 M08。

2026-10-22 已接入[创建时新建模型与 Unit 单层覆盖](openspec/changes/add-unit-model-overrides/proposal.md)：创建时搜索或新建模型，在模拟器配置表保存局部差异，支持恢复、选择性回写、另存独立模型及确认值捕获。保存／回写不自动应用；该阶段仅会话内保存；当前工程模式见 M08，不增加配置版本或模型继承链。[使用说明](docs/unit-model-overrides.md) · [实施与验证](docs/test-records/2026-10-22-unit-model-overrides.md)。


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

工作台外围已按确认方案整理：项目树、配置／运行操作、上下文属性、应用迁移预览和真实通信／事件／问题面板。点表保留 NovaSheet 样式；报文可筛选、暂停展示和导出当前视图（有界最近窗口，不是完整抓包）。见[UI 实施与原生截图](docs/test-records/2026-10-18-workbench-ui-v2.md)。M07历史查看与离线回放延期至最后，当前没有产品历史／回放入口。

通信报文字节现支持点击／Enter打开字段解析，覆盖基础8功能码的TCP请求、响应及异常；读响应仅在唯一匹配请求时补地址。“详细”进入Dockview中的[Modbus指南](workbench/src/resources/protocols/modbus/guide.md)，项目树“文档资源”与采集模型平级，按协议分类、离线Markdown渲染。见[本轮记录](docs/test-records/2026-10-18-modbus-reference.md)与[解析边界](docs/test-records/2026-10-18-message-hover.md)。


M05 TCP主站：单目标共享连接、多Unit独立结果、四区手动／周期读取、遥控／遥调显式下发及回验、质量／更新时间、采集中自动重连和真实字节观测。默认连接／请求／重连为3000／1000／3000ms，过期倍数3；主站设置进入配置草稿，保存并应用后生效。应用会断开并保持断开；待写草稿需明确丢弃。首版最多32个主站所有者槽，单批手动写入最多1024点（先整批校验，再按功能码上限拆请求），非主站规模性能承诺。RTU主站按独立提案接入软件运行；[实施与验收边界](docs/test-records/2026-10-19-m05-tcp-master.md)。

```sh
cargo build -p essim-infrastructure --features tcp-runtime,vendored --example tcp_master --offline
# 需要PyModbus 3.11.3；只访问脚本自己启动的回环从站。
python3 scripts/verify_tcp_master_runtime.py
```


串口路径支持系统设备下拉、刷新和手动输入，显示系统提供的设备名称与USB标识；发现使用 serialport 4.10.1，实际通信仍由 libmodbus 管理。刷新不会替换已有配置或打开串口。

M05 RTU主站软件已接入，复用主站调度和工作台，支持多目标共享独占串口、基础8功能码、手动受限串行诊断、CRC／短帧观测、有界迟到数据隔离和原路径自动重开。软件证据来自独立PyModbus PTY；原生与真实RS-485待验范围见[RTU主站记录](docs/test-records/2026-10-19-m05-rtu-master.md)。M04 RTU从站首批软件状态见下文；PTY不代替硬件验收。

```sh
cargo build -p essim-infrastructure --features tcp-runtime,vendored --example tcp_master --offline
# Python需pymodbus==3.11.3、pyserial==3.5；仅操作脚本创建的PTY。
python3 scripts/verify_rtu_master_runtime.py
```


M04 RTU从站首批软件已接入正式运行入口：单串口单站、基础8功能码、外部写权限、合法广播写入静默、置数／恢复初值、停启保值与实际RTU字节记录。工作台应用配置后点击“打开串口”，停止时点击“关闭串口”。仅08/0000回送及11标识诊断已支持，其他诊断返回01；实际RS485时序与方向仍待硬件验证。[验收边界](docs/test-records/2026-10-19-m04-rtu-slave.md)。

```sh
cargo build -p essim-infrastructure --features tcp-runtime,vendored --example tcp_slave --offline
# 需要pymodbus==3.11.3、pyserial==3.5；只使用脚本创建的PTY。
python scripts/verify_rtu_slave_runtime.py
```

独立PyModbus串口交互工具：[modbus_rtu_client.py](example/modbus_rtu_client.py)，支持串口选择／刷新／手动路径、四区读取及yk／yt单点／范围写入；参数与操作见[example说明](example/README.md#rtu-串口交互测试)。


M06已接入点位分组原子创建、固定范围的批量应用／启停、取消后续项、安全重试、Dockview任务与资源页、统一资源账本及有界可靠录制。导航与模型内容按需读取；关闭任务Tab不取消后台操作。配置仍仅保存当前内容，不提供历史版本或跨进程任务恢复。

[交付与验收记录](docs/test-records/2026-10-20-m06-delivery.md)保留容量边界：10端点×1Unit×100点完成30分钟10／5／1秒分阶段扫描，共7800请求、零错误／漏轮；1000个小表端点未达到1秒扫描；10000连接探测存在大量超时；典型一亿点组合被暂存预算预检拒绝，未运行。单Unit 100000点读取有独立短探测，不能外推组合容量。真实RS-485继续待硬件验证，不自动配置虚拟IP。

```sh
# 独立并发短探测，完整录制开启；结果COMPLETED仅表示测量结束，须检查错误和漏轮。
python scripts/measure_modbus_scale.py --binary target/debug/examples/tcp_slave --endpoints 10 --units 1 --points 100 --clients 1 --periods 10 5 1 --duration 10 --output /tmp/essim-scale.json
# 各阶段600秒，共30分钟；自行选择与本机预算相符的负载。
python scripts/measure_modbus_scale.py --binary target/debug/examples/tcp_slave --endpoints 10 --units 1 --points 100 --clients 1 --periods 10 5 1 --duration 600 --faults --output /tmp/essim-sustained.json
```

M07历史查看与离线回放已按用户要求移至实施顺序最后，尚未交付；M08本地存储已完成约定范围及 macOS 验收，不再列为下一项待办。现有实时事件／报文查看和可靠录制保留。

右侧属性范围已迁移到 Dockview 右边缘的点／Unit／端点三个图标标签，默认 Unit；顶部开关完全隐藏／恢复，其他布局保留。[提案与实现边界](docs/test-records/2026-10-23-right-edge-inspector-tabs.md)。

右侧属性区已应用 Blender 参考的紧凑密度：28px图标栏、选中与内容同色融合、22px常规字段行；其他布局不变。[验证与边界](docs/test-records/2026-10-23-compact-inspector.md)。

中央文档Tabs使用28px的D融合样式，选中与内容连通，「＋」紧跟最后一个标签。[样式及验证](docs/design/document-tabs.md)。

## 资源树组织与文档停靠

已接入多级文件夹、48款主题图标／颜色、目录内原子创建、多源换层级和中央Dockview文档拖放定位。中央文档空白已修复，核心与前端自动检查通过；用户已确认macOS原生树内多选换层级、循环拒绝及标签四向分屏／合并正常。原生树拖入中央分屏也已由用户确认，树归属和Tab唯一性保持；业务菜单键盘、弹框焦点／校验／忙碌保护及900×600原生滚动已验，鼠标八向缩放、窗口边界和复位补验通过，提案26/26项完成。见[接入说明](docs/design/resource-tree-docking.md)和[实施记录](docs/test-records/2026-10-24-resource-tree-docking.md)。


工程持久化已完成[方案提案](openspec/changes/persist-project-files-and-runtime-data/proposal.md)：使用通用 `project.workspace` 清单、独立静态资源文件和运行 SQLite，启动恢复最近工程但不自动运行；已在本分支实现并完成 macOS 流程与自动化验收，见[实施记录](docs/test-records/2026-10-24-project-persistence.md)。Windows／Linux 原生行为仍待验证。


## 应用诊断日志（提案中）

计划将应用错误与调试日志写入系统应用日志目录，支持结构化分级、大小轮转、有界后台写入、Rust／前端异常及日志故障可见性，并提供打开日志目录命令。独立于工程 `runs/`、SQLite 与协议可靠录制，不自动上传。本轮仅形成[提案](openspec/changes/add-application-diagnostic-logging/proposal.md)，尚未实现。


## 工程启动与切换（已实现，macOS 验收完成）

用户已确认取消正式客户端临时会话：首次无工程显示 960×720 固定尺寸的左右分区引导（小屏适配），必须打开或新建工程才进入工作台；打开后恢复工作台窗口边界；启动恢复最近工程，关闭工程返回引导。顶部调整为左侧工程菜单、中间历史与检索、右侧三个面板开关。按[提案](openspec/changes/require-project-startup-and-switcher/proposal.md)，已接入工程门禁、独立引导页、最近列表及标题栏布局；自动化和 macOS 流程已验证，标题栏拖动由用户手工确认；Windows／Linux 原生行为待验。


## 多工程独立窗口（已实现，macOS 验收完成，2026-10-25）

用户已确认同进程、每窗口一个工程：默认打开／新建使用新窗口，已打开工程直接恢复并激活其窗口；工程状态、通信、草稿和后台任务按窗口隔离。关闭窗口只影响自身，退出应用统一保护全部工程；启动仍只恢复最近活动的一个工程。独立 worktree 已接入，真实双工程通信和 macOS 主流程已验证；最小化恢复原窗口且无重复已由用户手工确认，22/22 项完成，Windows／Linux 原生行为仍待验。见[提案](openspec/changes/open-projects-in-independent-windows/proposal.md)与[实施记录](docs/test-records/2026-10-25-multi-project-windows.md)。工程格式、保存／显式应用和已有文件锁不变。

## 应用诊断日志

已接入独立系统目录的分级 JSONL 日志、进程会话锁、大小轮转、有界非阻塞队列、前后端异常和两秒退出刷新。顶部命令 `>打开应用日志目录` 打开固定目录，“问题”面板显示日志降级；同进程全部工程窗口共享，工程通信记录保持独立。macOS 已验，Windows／Linux 原生待验。见[使用说明](docs/operations/application-logging.md)与[验收记录](docs/test-records/2026-10-25-application-logging.md)。


## IEC 101／104 协议库基础

已建立独立的 [lib60870 三包](crates/lib60870/README.md)：`lib60870-src` 保存完整官方 v2.4.1 源码并离线构建，`lib60870-sys` 管理原始 FFI／链接，`lib60870-rs` 提供首批线程绑定的 ASDU、单点／浮点遥测及 104 无线程服务端安全 API。101 源码完整保留，101 安全通信 API、104 客户端、TLS 与其他类型尚未封装。

本阶段不接入工作台、不修改 Modbus／Tokio 执行模型。封装验证范围与命令见[说明](crates/lib60870/README.md)和[实施记录](docs/test-records/2026-10-25-lib60870-packages.md)，不将库测试描述成真实 EMS、完整协议或规模验收。
