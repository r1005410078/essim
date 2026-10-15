# libmodbus Rust 封装

三个私有包同属 ESSim 根 Cargo workspace，只有一份 Cargo.lock；此目录不是嵌套 workspace。

| 包 | 职责 |
| --- | --- |
| libmodbus-src | 固定官方源码、离线构建辅助 API、静态/动态产物与清单 |
| libmodbus-sys | 原始 FFI 子集、版本/ABI 探针、显式来源选择及链接元数据 |
| libmodbus-rs | 通用安全 API、错误、context/mapping RAII 与 socket 生命周期 |

依赖方向为 `infrastructure → rs → sys`，sys 的 vendored 构建依赖 src。三个库不依赖 ESSim 核心、Tauri 或 UI。当前覆盖 TCP 服务端、RTU 串口从站及四类数据区映射所需子集，不是完整 libmodbus API；不含主站功能。源码版本固定为 3.2.0，来源/校验见 [SOURCE.md](libmodbus-src/SOURCE.md)。

## 模式约定

| 配置 | 行为 |
| --- | --- |
| rs 默认；ESSim/Tauri 默认 | vendored：内置源码编译并静态链接 |
| sys 单独使用 | 默认无 feature，需主动开启 vendored 或给动态前缀 |
| 关闭默认 feature + LIBMODBUS_DIR | 显式链接自行编译的动态库 |
| vendored + LIBMODBUS_DIR | 冲突报错 |
| 关闭 vendored 且无有效前缀 | 报错，不找宿主机库 |
| LIBMODBUS_PREFIX | 已废弃，提示迁移 |

不自动使用 pkg-config、Homebrew、vcpkg 或宿主机 libmodbus。`vendored` 不表示动态模式；这里沿用 rust-openssl 的源码静态含义。动态构建必须先显式调用 src 的 build-shared 入口。

## 构建工具

需要 Rust/Cargo、C/C++ 编译器、make、autoconf、automake 和 GNU libtool。macOS 工具准备可执行 `brew install autoconf automake libtool`（开发工具，不是安装 libmodbus）；还需可用的 Xcode Command Line Tools。macOS 使用 glibtoolize，Linux 使用 libtoolize。

正常 Cargo 构建不联网、不安装系统包，不修改 vendor；只在私有输出目录复制源码、生成 configure/config.h 并编译安装。产物按 target/模式/profile/源码与编译设置隔离，提供 build.log 与 libmodbus-build.json。首次依赖获取与开发工具准备仍需单独完成；`--offline` 要求 Rust 依赖已缓存。

## 已实测命令（macOS arm64）

根目录执行默认静态测试和普通寄存器示例：

```sh
cargo test --workspace --offline
cargo run -p libmodbus-rs --example register_demo --offline
cargo run -p libmodbus-sys --features vendored --example version --offline
cargo run -p essim-infrastructure --example modbus_demo --offline -- 1502
```

动态库先自行编译：

```sh
cargo run -p libmodbus-src --example build-shared --offline -- target/libmodbus-shared
```

入口打印 `LIBMODBUS_DIR=...`。本机结果如下；其他 target 使用实际输出的绝对前缀，不照抄架构名称：

```sh
export LIBMODBUS_DIR="$PWD/target/libmodbus-shared/aarch64-apple-darwin-Dynamic-debug/install"
cargo test --workspace --no-default-features --offline
cargo run -p libmodbus-rs --no-default-features --example register_demo --offline
cargo run -p libmodbus-sys --no-default-features --example version --offline
cargo run -p essim-infrastructure --no-default-features --example modbus_demo --offline -- 1502
```

切回静态前移除当前终端设置的 LIBMODBUS_DIR：`unset LIBMODBUS_DIR`。基础设施和 Tauri 各自转发 vendored；workspace 中依赖声明使用 default-features=false，避免动态入口被下游默认 feature 重新开启。

## 安全 API 的范围

`Mapping` 支持线圈、离散输入、保持寄存器和输入寄存器，各区使用独立的 `Option<AddressRange>` 配置；`None` 不分配。`AddressRange::new(start,count)` 校验非空区间和 0–65535 地址范围。位区本地 API 使用 `bool`，寄存器使用 `u16`；输入区允许服务端内部更新，网络侧为只读。FC01/02/03/04 读取和 FC05/06/0F/10 写入通过 libmodbus 应答。

`HoldingRegisters::new(start,count)` 保留为兼容入口，仅分配保持寄存器区；它通过 Deref 转换为 `Mapping` 传给 `reply`。它校验地址区间，`set/get` 校验边界，数值范围为完整 UInt16。`TcpContext::with_socket(owner_or_guard,timeout)` 接收 Rust socket owner 或带生命周期的 BorrowedFd。C 只借用 fd，context Drop 先脱离并释放 C 对象，随后 Rust socket owner 才析构；无重复 close。context/mapping 不实现 Send/Sync，无 C 回调。

`Request` 只有成功 receive 才能生成，safe reply 不接受任意切片。libmodbus 接收仍有自己的分帧语义，不等于恶意 TCP 帧防火墙。ESSim 外层负责 MBAP/PDU 校验、缓冲上限、内部 socketpair、只读策略、worker、停机和报文。地址 100、初值 650、SOC 范围均保留在 ESSim。

## macOS 本机验证包

```sh
python3 scripts/bundle_macos.py static
python3 scripts/bundle_macos.py dynamic --libmodbus-dir "$PWD/target/libmodbus-shared/aarch64-apple-darwin-Dynamic-debug/install"
```

分别输出 `target/artifacts/static/ESSim.app`、`target/artifacts/dynamic/ESSim.app`。动态模式携带本次编译 dylib，使用 `@executable_path/../Frameworks`，移除开发前缀 rpath；脚本进行本机 ad-hoc 签名及验证。静态 libmodbus 不等于程序完全静态，仍依赖系统框架。

这是开发验证包，不是经过开发者身份签名、公证或完成公开分发准备的安装包。许可副本和源码来源随包放在 Resources/third-party/libmodbus。

## 平台边界

| 平台 | 状态 |
| --- | --- |
| macOS 26.4.1 / aarch64-apple-darwin | 静态/动态、Rust 测试、headless、Tauri/NovaSheet 与独立 PyModbus 验收 |
| Linux GNU（本机 target） | 构建路径采用上游 Autotools，未实测；加载路径、libc 与部署另验 |
| Windows MSVC/MinGW | 当前明确拒绝；需实现上游 configure.js/工具链对应路径及 socket/导出/CRT 适配 |
| 交叉编译、macOS universal、musl | 当前明确拒绝，不将 HOST 产物当成 TARGET |

上游 Windows 3.2.0 曾有 SOCKET/FD_SETSIZE 报告（#863）；本轮不修改上游或把 SOCKET 强转为 i32。Linux 没有因为共享 Unix API 就被标为已验收。原三包拆分未做 RTU 验收；下文补充了 RTU 软件验证，真实 EMS、串口/RS-485 硬件、规模与完整 API 兼容性仍未验收。

## 许可与后续发布

上游 LGPL-2.1-or-later 原文、AUTHORS 和完整源码保留于 vendor，未应用补丁。两种模式的后续公开分发均需核实许可、对应源码/修改及适用安排。静态模式还须准备适用条款下重新链接所需材料或其他合规方案；动态模式须核实替换库的安排。

本轮记录了这些发布事项，不宣称源码进入仓库或应用 ad-hoc 签名就完成全部分发要求。包装层的公开名称、许可证和版本发布另定；目前 publish=false。

四类数据区封装的本轮验证见 [四区验收记录](../../docs/test-records/2026-10-15-libmodbus-four-areas.md)；静态/动态测试通过，不代表界面点表或新增独立客户端互操作已完成。

四区命名统一为 `holding`（保持寄存器）、`input`（输入寄存器）、`coil`（线圈）、`discrete`（离散输入）；Mapping 的本地访问方法使用这些名称及 `set_` 前缀。构造参数顺序沿用 C 映射顺序 coil、discrete、holding、input，不因显示顺序调整。上游 C API 和 `HoldingRegisters` 兼容入口保留原名。

## RTU 串口从站

`RtuConfig::new(device, baud, parity, data_bits, stop_bits, slave, receive_timeout_ms, byte_timeout_ms)` 校验配置；`RtuContext::open(&config)` 实际打开串口。`receive()` 返回 `Result<Option<RtuRequest>>`：Some 为结构检查后的有效请求，None 为上游忽略，错误区分超时、协议错误和串口错误。`reply` 接受同一四区 `Mapping`；独立的 `RtuRequest` 不可传入 TCP reply，也不能用 TCP Request 调用 RTU。

首版支持列表：

| 参数 | 实现接受的范围 |
| --- | --- |
| 波特率 | 9600、19200、38400、57600、115200 |
| 数据位 | 8 |
| 校验位 | N、E、O |
| 停止位 | 1、2 |
| 从站号 | 1–247；0 仅作为收到的广播 |
| 等待请求/忽略确认超时 | 10–100ms |
| 字节间超时 | 1–2ms |

该列表是当前软件配置子集，不是硬件验收矩阵；本轮 PTY 仅实测 19200/N/8/2、50ms/2ms。串口驱动可拒绝组合，硬件参数和时序必须另验。有限等待和 256 字节上限约束单次接收阻塞，未支持更低波特率或任意长超时。

RTU 的串口由 C context 打开并持有，Drop 按实际打开状态 close → free；与 TCP 中 Rust 持有 socket、C 借用 fd 的方式不同。Unix 上游使用 O_NONBLOCK、默认 RTS_NONE，封装不启用自动重连/链路重试或自定义方向回调；发送受阻返回错误，不无限等待。通用正常 reply 可执行合法广播写入，但不回复；异常 reply 在 Rust 侧拦截广播，避免上游异常函数发送广播响应。

通用封装不监听应用命令或解释 SOC；实际配置、停机、只读策略在 ESSim。RTU receive 使用官方库的帧语义，不宣称是完整恶意串口流防火墙或 RTU 时序符合性认证。完整软件验收与硬件边界见 [RTU 验收记录](../../docs/test-records/2026-10-15-rtu.md)。


## TCP / RTU 主站

`ModbusClient` 是同步主站：主动连接从站，再发起读写；不需要本地 `Mapping`。
`Mapping` 保存从站本地数据，`ModbusClient::read_*` / `write_*` 则实际发送请求。
全部地址为零基协议地址，数值和倍率由调用方解释。以下地址 100 为教学约定：

```rust
use libmodbus_rs::ModbusClient;
use std::time::Duration;
fn read_device() -> std::io::Result<()> {
let mut client = ModbusClient::tcp(
    "127.0.0.1:1502".parse().unwrap(), 7,
    Duration::from_millis(500), // TCP 连接等待
    Duration::from_millis(500), // 响应等待
)?;
let values = client.read_holding(100, 2)?;
// client.write_holding(100, 12)?; // 会写远端，仅用于允许写入的点
Ok(())
}
```

RTU 使用 `ModbusClient::rtu(&RtuConfig::new(path,19200,'N',8,2,7,50,2)?, Duration::from_millis(500))?`。
串口参数子集与从站一致；配置中 50/2ms 是现有从站等待字段，主站会覆盖为独立响应等待并禁用 byte_timeout。
TCP 的连接/写入等待和 C 响应等待分别设置；RTU 串口由 C 打开，按 close → free 释放。

| 方法 | 数据区 / 功能码 | 数量上限 |
| --- | --- | --- |
| `read_coil` / `read_discrete` | coil / discrete，01 / 02，返回 Vec<bool> | 2000 |
| `read_holding` / `read_input` | holding / input，03 / 04，返回 Vec<u16> | 125 |
| `write_coil` / `write_holding` | coil / holding，05 / 06 | 1 |
| `write_coils` / `write_holdings` | coil / holding，0F / 10 | 1968 / 123 |

`set_unit` 切换后续目标，TCP 允许 0–255，RTU 允许 1–247，不提供广播。
区间为空、超过功能码数量或末地址越界时发送前返回 InvalidInput。`exception_code(&error)`
提取保留 errno 中的远端异常码；合法异常后可以继续请求。I/O、超时或上游检测到的无效确认
立即释放通道，后续返回 NotConnected，须显式重建；没有自动重连/重试。写入超时可能已执行，
不能当作“设备没有动作”。重新打开 RTU 也不保证迟到响应已经从总线消失，调用方仍须协调恢复。

上游 3.2.0 TCP 确认检查事务号、协议号、功能码及 PDU 数据，但不严格检查响应 Unit ID 和
MBAP 长度；此封装沿用该边界，不作为不可信从站协议防火墙。response_timeout 是上游接收等待语义，
底层 select 对 timeval 的处理依平台而异，不承诺整个请求的墙钟截止时间。
当前无主站轮询调度或工作台主站 UI；真实设备、RS-485 电气/方向/时序未验。

构建并连接独立测试从站（示例会写入教学点，不用于生产设备）：

```sh
cargo build -p libmodbus-rs --example client_demo --offline
# Python 环境需固定 pymodbus==3.11.3、pyserial==3.5
python3 scripts/verify_modbus_master.py
# 也可自行启动相同测试点表的从站后运行：
cargo run -p libmodbus-rs --example client_demo --offline -- tcp 127.0.0.1:1502
cargo run -p libmodbus-rs --example client_demo --offline -- rtu /dev/serial-example
```

`verify_modbus_master.py` 自动建立独立 TCP 和 PTY RTU 从站；数据区零基地址100–101，
初始 coil=[true,false]、discrete=[false,true]、holding=[42,43]、input=[99,100]，站号7，
串口19200/N/8/2。可用 `--binary` 指定已构建示例。显式动态模式构建命令见上文，
同一脚本适用于两种模式；实际报文和验证范围见 [主站验收记录](../../docs/test-records/2026-10-15-modbus-master.md)。
