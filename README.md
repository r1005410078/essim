# ESSim

ESSim 是面向储能通信协议的多设备模拟工具，用于协议开发、设备联调和场景验证。

项目同时作为协议学习实验室：从零理解储能设备与通信角色，通过报文分析、客户端与服务端实现、故障实验和独立互操作验证，逐步建立协议开发与排障能力。

项目方向：[项目目标与使用场景](docs/project-goals.md) · [AI 开发约束](AGENTS.md)。

学习入口：[学习与开发路线](docs/learning-roadmap.md) · [第一课：一次 SOC 读取](docs/lessons/01-read-soc.md) · [Modbus TCP 笔记](docs/notes/modbus-tcp.md)。

产品技术方案暂定为 Rust + Tauri + TypeScript/React 桌面应用，协议层优先封装成熟 C 协议库。选型依据是产品需求，而非现有 Rust 学习代码；已有 Excel 风格组件将在核实接口及系统 WebView 兼容性后接入。

工具的目标是同时支持客户端模拟、服务端模拟和网络条件模拟，让多个模拟设备能够在同一个场景中运行，也能与真实设备或外部系统通信。

## 目标能力

- **客户端模拟**：主动连接目标服务端，按照所选协议发起请求、发送数据并处理响应。
- **服务端模拟**：监听连接，接收外部客户端请求，按照设备模型和协议规则返回数据。
- **多设备模拟**：独立配置每个设备的身份、协议、连接参数、运行状态和数据；同一场景可以混合运行客户端与服务端。
- **网络模拟**：配置延迟、抖动、丢包、限速、断连与恢复等条件，验证通信异常时的行为；具体实现需要结合传输方式确定。
- **场景运行与观测**：组织设备和通信关系，控制运行过程，记录协议报文、设备状态及异常事件。

## 建议的核心模块

| 模块 | 职责 |
| --- | --- |
| 设备模型 | 管理设备身份、数据点、状态及行为 |
| 协议适配 | 处理报文编解码、请求响应及协议交互规则 |
| 通信传输 | 管理连接、监听、收发及连接生命周期 |
| 网络模拟 | 在通信链路中施加可配置的网络条件 |
| 场景管理 | 配置多设备及其通信关系，控制启动与停止 |
| 运行记录 | 记录报文、事件和运行结果，辅助联调与分析 |

客户端与服务端是通信角色，应与设备类型分开配置。设备模型、协议适配和通信传输应保持独立，便于复用同一设备模型并扩展不同协议。

## 当前状态

当前保留 `prototype/` 中的 React/Vite/Tauri 界面原型，包含项目文件树、点表模型和本地模拟交互。根目录旧 Rust 学习原型已移除；正式 Rust workspace 已建立，并完成单实例 Modbus TCP 教学 SOC 闭环；`prototype/` 仍只表达本地业务交互。

建议以 Modbus TCP 和一个教学用电池设备为首个开发阶段，打通一次 SOC 读取，再逐步增加写入、异常处理、多设备和网络条件模拟。详细阶段与验收条件见学习路线；第一课读取与异常分支已有独立客户端证据，其余课程仍为计划。

## 桌面界面原型

[prototype](prototype/README.md) 已改为 Dockview 停靠式工作台，旧卡片界面已移除。设备维度覆盖点表库、设备与通讯树、3D 视图和配置；策略维度提供脚本编辑和简化离线仿真。运行步骤与限制见 [用户故事](docs/prototype-user-stories.md)。该原型尚未接入真实协议引擎。

## 自定义项目文件树原型

设备工作区改为用户自定义文件夹结构。右键或树顶部加号可新建文件夹、Modbus 点表模型、Modbus 主站/从站模拟器；主从角色由入口确定。文件及文件夹支持重命名、移动与删除，非空目录和被引用的模型暂不允许删除。移动和重命名不改变稳定 ID 引用。

点表模型复用现有表格编辑器；模拟器复用设备与通讯配置页面，创建弹框配置 Modbus TCP 地址、端口、Unit ID 和模型引用。两种文件通过右键导出带版本的 JSON，模拟器导出附带引用模型。结构通过“保存项目”保存至浏览器本地工作区；配置导入、真实文件系统同步、RTU 通信与真实协议运行尚未实现。

## 紧凑点表工作区与工具栏

点击模拟器直接打开复用 NovaSheet 的四遥运行点表，通信参数、模型引用及应用配置放在右侧紧凑属性栏，中央不再展示总览、逐行操作按钮。工具栏仅显示图标，悬停提示用途：从站为启动/停止、批量置数、恢复选区初始值、导出；主站为连接/断开、读取选区、写入选区、导出；点表模型为保存、撤销/重做、新增/删除点、JSON 导入/导出。复制粘贴保留表格快捷键及右键入口。

主站编辑控制值仅修改表格草稿，显式写入选区后才更新本地模拟对端；选区先整组校验，失败不更新运行状态。从站批量置数与恢复仅作用于选区；模型导入先替换编辑草稿，保存后更新共享模型。`prototype/` 中启动、连接和读写均为会话内本地模拟，不建立网络连接；RTU、真实协议互操作及系统 WebView 表格兼容性仍未验证。

## 正式工程

`workbench/` 为正式 TypeScript/React/Tauri 工作台，`packages/` 保存共享 UI 与类型配置，`crates/` 为独立 Rust 核心与基础设施。已接入 libmodbus 3.2.0（默认 vendored 源码静态链接，支持显式自行编译动态库），支持一个 TCP 端点下动态添加 Unit ID、各单元独立教学 SOC 的读取/内部更新、异常响应、有界报文记录和真实停机。工作台表格复用 NovaSheet。当前验证平台为 macOS；RTU 单串口单从站的软件闭环已接入（PTY 验证），真实串口/RS-485 硬件、通用点表、多设备、SQLite 和真实 EMS 接入尚未验收。

```sh
pnpm install
pnpm dev       # 浏览器预览，端口 1430，无后端时明确提示
pnpm desktop   # Tauri 桌面开发
pnpm typecheck
pnpm build
pnpm test
cargo check --workspace
cargo test --workspace
```

原型仍使用 `prototype/` 内原有命令，未加入正式前端 workspace。详细架构见 [正式工程说明](docs/architecture/workspace.md)。

共享 shadcn/ui 已配置，添加组件执行 `pnpm ui:add <组件名>`；组件位于 `packages/ui/src/components`。

## libmodbus 首个闭环

已建立 [libmodbus TCP 最小闭环提案](openspec/changes/integrate-libmodbus-tcp-loop/proposal.md)，包含安全封装、单从站 SOC 读取与运行值更新、工作台接入、异常及独立互操作验收。实施和验收记录见 [Modbus 闭环记录](docs/test-records/2026-10-14-modbus-loop.md)，依赖来源/许可与平台边界见 [libmodbus 依赖说明](docs/test-records/libmodbus-dependency.md)。

默认端点 `127.0.0.1:1502`、Unit ID `1`、Holding Register 零起始地址 `100`，原始值 `650` 表示 `65.0%`，允许整数 `0–1000`。这些是教学约定。工作台通过 Unit ID 下拉框选择单元，在旁边输入新 ID 并点击 ＋ 添加（本阶段范围 1–247，停止或监听中均可添加）。每个单元的 SOC 初始值为 650，草稿与生效值独立；编辑草稿后按 Enter，再点击工具栏 ✓ 应用到选中单元。配置与运行值在本次应用会话停启间保留，退出应用后不持久化。

无界面入口复用同一应用服务：

```sh
cargo run -p essim-infrastructure --example modbus_demo -- 1502
# 输入 700 并回车更新，输入 quit 停机；stdin EOF 也会停机。
```

构建默认从内置的固定 libmodbus 源码静态编译，需要 C/C++ 与 Autotools 开发工具。动态模式必须显式设置自行编译产物的 `LIBMODBUS_DIR`；不再自动使用 Homebrew 或 `LIBMODBUS_PREFIX`。详见 [三包构建说明](crates/libmodbus/README.md)。独立验证脚本 `scripts/verify_modbus.py` 要求 Python ≥3.10 和 PyModbus 3.11.3；使用 `--expected 650` / `--expected 700` 分别验证更新前后。

## libmodbus 三包封装

`crates/libmodbus/` 下的 `libmodbus-src`、`libmodbus-sys`、`libmodbus-rs` 已接入同一根 Cargo workspace。默认 vendored 从内置官方源码编译并静态链接；非 vendored 使用显式自行编译动态前缀，不自动查找宿主机 libmodbus。通用封装现支持线圈、离散输入、保持寄存器、输入寄存器四区映射；正式 SOC 闭环每个 Unit ID 仍只使用一个保持寄存器，通用点表配置尚未接入。两个模式在 macOS 完成 Rust、headless、Tauri/NovaSheet 和 PyModbus 读回验证；其他平台和公开分发准备仍有明确边界。

[构建与使用](crates/libmodbus/README.md) · [验收记录](docs/test-records/2026-10-15-libmodbus-workspace.md) · [提案与任务](openspec/changes/extract-libmodbus-workspace/proposal.md)。

动态 Unit ID 的配置、生效方式与验证边界见 [验收记录](docs/test-records/2026-10-15-dynamic-unit-id.md)。独立验证脚本可用 `--unit-id 2` 指定目标单元，默认仍为 1。

## RTU 串口从站

正式工作台通信栏可切换 TCP / RTU。RTU 首阶段为单串口、单从站：填写真实串口路径、波特率、校验位、8 数据位、停止位和站号；停止后点击工具栏“应用通信配置”或直接启动。启动成功表示串口实际打开，运行时禁止修改通信参数。SOC 仍为教学 holding 地址 100，初值 650，外部只读；编辑后按 Enter，再点击 ✓ 确认内部更新。

TCP 与 RTU 各自保留配置草稿和运行值，TCP 原有动态 Unit ID 不用于同一串口多站仿真。首版波特率为 9600/19200/38400/57600/115200，站号 1–247，请求等待 10–100ms、字节间等待 1–2ms。参数范围及虚拟环境限制见 [封装说明](crates/libmodbus/README.md)。本轮只在 macOS PTY 上实测 19200/N/8/2，不将参数被接受写成真实硬件通过。

无界面入口使用同一应用服务：

```sh
# /dev/serial-example 为教学占位路径，须替换；最后的 50 为接收等待毫秒。
cargo run -p essim-infrastructure --example rtu_demo --offline -- /dev/serial-example 19200 N 8 2 7 50
# 输入 700 更新，quit 或 stdin EOF 停机。
```

独立软件互操作采用两对 PTY 转发，Python ≥3.10 环境需要 PyModbus 3.11.3 和 pyserial 3.5：

```sh
cargo build -p essim-infrastructure --example rtu_demo --offline
python3 scripts/verify_rtu.py --binary target/debug/examples/rtu_demo
# 或打印临时串口路径，在工作台填写并启动后，向验证脚本输入 650 / 700：
python3 scripts/verify_rtu.py --desktop
```

静态/动态的 Rust 与独立客户端证据、桌面流程及真实硬件待验事项见 [RTU 验收记录](docs/test-records/2026-10-15-rtu.md)。


## Modbus 通用主站库

`libmodbus-rs::ModbusClient` 已提供 TCP 主动连接和 RTU 串口主站，支持四区读取、线圈/保持寄存器单写与多写、目标站号切换、异常码与失败后关闭通道。工作台当前仍为从站仿真，尚未接入主站 UI 或轮询调度。

```sh
cargo build -p libmodbus-rs --example client_demo --offline
# 使用装有 pymodbus==3.11.3、pyserial==3.5 的 Python：
python3 scripts/verify_modbus_master.py
```

脚本自动启动独立 PyModbus TCP/PTY RTU 测试从站；示例会写入教学点。用法、数量上限及上游 TCP 确认校验/超时边界见 [封装说明](crates/libmodbus/README.md)，实测范围见 [主站验收记录](docs/test-records/2026-10-15-modbus-master.md)。真实设备及 RS-485 硬件尚未验证。
