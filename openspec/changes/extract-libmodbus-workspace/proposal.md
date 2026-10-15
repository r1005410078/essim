## Why

现有 libmodbus 构建、FFI、安全管理和教学 SOC 策略混在 ESSim 基础设施中，并依赖宿主机 Homebrew 动态库。需要将其拆成可独立复用的源码构建、原始绑定和安全封装三个包，同时保持已经验证的真实 TCP 闭环。

## What Changes

- 在现有根 Cargo workspace 内新增归类目录 `crates/libmodbus/`，包含 `libmodbus-src`、`libmodbus-sys`、`libmodbus-rs` 及共用 README；不创建嵌套 workspace。
- `src` 保存固定官方源码、来源/校验信息和补丁，提供显式的静态/动态编译能力；构建不联网、不安装系统库。
- `sys` 提供当前 API 子集的原始绑定、ABI/版本检查与链接配置；`vendored` 调用 `src` 编译并静态链接，沿用 rust-openssl 的熟悉含义。
- `rs` 提供通用 Rust 安全 API、RAII 和错误转换，并转发 `vendored`；不包含 SOC、设备或 Tauri 业务。
- 默认使用 `vendored` 静态模式；关闭它时必须显式提供我们自行编译的动态库前缀。不得自动探测 Homebrew、pkg-config、vcpkg 或静默回退宿主机 libmodbus。
- **BREAKING（开发构建约定）**：替换基础设施中的 C 编译与私有 FFI；旧 `LIBMODBUS_PREFIX` 开发路径迁移为新包明确的 `LIBMODBUS_DIR` 契约。原拆分阶段运行点表、IPC 和协议行为保持不变；后续动态 Unit ID 的界面和 IPC 扩展见文末补充。
- macOS 验收两种模式、无界面/桌面与独立客户端读回；Linux/Windows 声明目标与构建路径，未执行的平台不标为支持完成。
- 补充 RTU 串口从站计划：增加通用安全封装、串口参数配置、无界面入口与工作台 TCP/RTU 选择，复用现有四区 Mapping；软件已实施，真实串口/RS-485 硬件待验。

## Capabilities

### New Capabilities

- `libmodbus-source-build`: 固定官方源码、离线构建、静态/动态产物和来源清单。
- `libmodbus-ffi-layer`: 原始 FFI、链接选择、feature/路径/版本与 ABI 契约。
- `libmodbus-rust-wrapper`: 无业务依赖的安全封装、资源/线程/socket 生命周期与请求安全。
- `libmodbus-essim-integration`: 三包在现有 workspace 的依赖方向、业务迁移及两种模式下原闭环回归。

### Modified Capabilities

无。当前 `openspec/specs/` 尚无已归档主规格；已完成的 `integrate-libmodbus-tcp-loop` 仍为独立变更。本提案引用其验收基线，不重写其历史工件或自动归档。

## Impact

未来实施涉及根 Cargo 清单/锁文件、新增三个 crate、基础设施构建与 native 封装、Tauri feature/打包配置、测试和文档。原拆分阶段无需改变前端 NovaSheet、业务 IPC、核心 SOC 规则和 `prototype/`；后续动态 Unit ID 与 RTU 接入会扩展正式工作台通信配置及 IPC，不改原型。新增 C 工具链构建要求与许可交付记录；纳入 RTU 从站扩展计划，增加通用主站子集，不包含工作台主站 UI、完整 API 覆盖、公开发布到 crates.io 或其他协议库。

三包拆分及后续四区、动态 Unit ID 已实施；RTU 串口从站的软件部分已实施，见 tasks.md 第 9 节及 docs/test-records/2026-10-15-rtu.md，真实串口/RS-485 硬件任务 9.9 仍未完成。旧 TCP 验收记录不作为 RTU 证据；尚未执行归档。

## 补充：四类数据区

新增 AddressRange 与 Mapping，分别配置线圈、离散输入、保持寄存器、输入寄存器的可选零基地址区间；位值使用 bool，寄存器使用 u16。C 辅助函数校验数据区和访问边界，Rust 不解引用 C 布局。保留 HoldingRegisters 兼容入口与 RAII/线程限制。验证 FC01/02/03/04/05/06/0F/10、区间边界、同地址隔离和输入区不被网络写入改变。当前 SOC 示例、IPC 与界面点表不扩展；不宣称新增真实 EMS 或独立工具验收。

补充项已实施，静态/动态封装与原 SOC TCP 回归通过；详见 `docs/test-records/2026-10-15-libmodbus-four-areas.md`，不将旧桌面和独立客户端记录算作新增四区验收。

## 补充：动态 Unit ID

同一 TCP 端点下可在停止或监听状态新增 Unit ID，本阶段产品范围为 1–247。默认保留单元 1，每个单元拥有独立 SOC 初值 650 和独立 libmodbus context/mapping，由同一 worker 串行管理。按请求 Unit ID 路由到对应映射；未知 ID 沿用关闭连接行为。新增及更新通过命令确认，映射创建成功后才发布完整快照；重复/非法 ID 拒绝。停止后再启动保留本会话配置和值，不提供删除、磁盘持久化或规模承诺。

界面提供 Unit ID 下拉框和数字输入框旁的添加按钮，新增成功后选中新单元；不同单元保留独立草稿，日志快照不覆盖草稿。IPC 和统一客户端支持 addUnit 与携带 unitId 的 updateSoc；旧更新入口默认作用于 Unit ID 1。仿真核心仍不依赖 Tauri，libmodbus-rs 不承担单元集合及设备调度。

## 补充：RTU 串口从站（软件已实现，硬件待验）

`libmodbus-rs` 新增 `RtuConfig`、`RtuContext` 与区分 TCP/RTU 的请求类型，复用 `Mapping` 的 holding、input、coil、discrete 四区。`libmodbus-sys` 补充固定 3.2.0 的 RTU 创建、连接、站号设置及关闭 API 和 ABI 探针。串口由 C context 打开并持有，正常停机及初始化失败均按实际获取资源执行 close → free；不复用 TCP 的 Rust socket 借用释放方式。

正式工作台提供 TCP/RTU 选择和串口路径、波特率、校验位、数据位、停止位、站号与超时配置；通过统一客户端及 IPC 调用核心。首阶段 RTU 为单串口、单从站（站号 1–247），复用教学 SOC 的读取和内部更新；停止后才允许应用通信参数。TCP 的动态多 Unit ID 继续保留，仅适用于 TCP。工作台 RTU 主站、同一串口模拟多个从站、RS-485 自定义方向控制及自动端口枚举留待后续。

RTU 不使用 MBAP/TCP 分帧与内部 socketpair，CRC 和收发交给 libmodbus；非本站及无效请求不应答，广播不得发送正常或异常响应。通用库合法广播写入与 ESSim 只读 SOC 拒绝策略分别验证。收发超时、串口断开和停机有独立验证任务；虚拟串口、独立客户端与真实 RS-485 的电气/时序验收分别记录。未完成硬件验证时明确标记，不将 PTY 互通写成硬件通过。

软件实现与静态/动态、PTY 独立客户端及桌面闭环已验证，详见 `docs/test-records/2026-10-15-rtu.md`；首版参数子集和未验事项以该记录为准，不能将软件验证标为硬件通过。


## 补充：TCP / RTU 通用主站

按用户“继续主站”指示，扩展 libmodbus-rs 的同步 ModbusClient：主动 TCP 连接、打开 RTU 串口、目标站号切换、四区读取和线圈/保持寄存器单写/多写。地址使用零基，发送前校验范围和功能码数量上限。返回原始 bool/u16，不加入设备业务或调度。

连接/响应等待明确配置，无自动重连或重试。合法 Modbus 异常保留 errno 并允许继续请求；超时、I/O 和被上游检测出的无效确认关闭通道，后续须重建。写入失败可能已在远端执行。TCP 确认沿用 3.2.0 上游校验，其响应 Unit ID/MBAP 长度校验不足及依赖平台的超时行为必须记录。本阶段不新增工作台主站 UI、广播、采集轮询、真实设备或 RS-485 硬件验收。

通用主站子集及两种模式的软件验收已完成，见 [主站记录](../../../docs/test-records/2026-10-15-modbus-master.md)；工作台主站和真实硬件仍不在已验收范围。
