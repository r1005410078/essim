## Context

原拆分前的 `crates/infrastructure/build.rs` 查找 `/opt/homebrew` 或 `LIBMODBUS_PREFIX`，编译 `native/modbus_shim.c` 并动态链接 3.2.0。该 C 薄层同时负责对象、地址 100/初值 650、功能码和只读策略；`protocols/modbus/native.rs` 用内部 Unix socketpair 获取库生成的真实响应。已有 macOS TCP、PyModbus 和 Tauri 验收记录。

用户已确认采用三包分层、同一根 workspace 下归类目录、熟悉的 `vendored=内置源码编译+静态链接` 含义。本文记录重构设计；实施进度见 tasks.md，实际验证见对应测试记录。不会改写已完成提案的历史证据。

## Goals / Non-Goals

**Goals:**

- 独立的源码构建、FFI、安全 API 三层，不依赖 ESSim 核心、Tauri 或界面。
- 固定上游 libmodbus 3.2.0；源码随项目保存，常规构建不联网、不全局安装依赖。
- 静态默认路径与自行编译动态库路径均能复现并验证，不自动采用宿主机 libmodbus。
- 保持现有 TCP 运行值确认、报文、订阅和停机语义；将 unsafe 集中到通用封装。
- 在补充阶段增加 RTU 串口从站，复用四区 Mapping，明确串口资源、请求类型、广播与停机边界。

**Non-Goals:**

- 完整 libmodbus API、工作台主站 UI/轮询调度、RTU 同一串口多从站仿真、通用 TCP 框架、异步运行时封装、设备模型扩展。
- crates.io 发布、拆出新仓库、嵌套 workspace、通信配置接入以外的前端改造或其他协议库迁移。
- 宣称 libmodbus 静态链接等于程序完全静态；宣称未经实测的 Linux/Windows 支持。

## Decisions

### 1. 一个 workspace、一个归类目录

```text
Cargo.toml                         # 唯一 workspace
crates/libmodbus/
  README.md                        # 模式矩阵、构建使用、版本/平台/许可
  libmodbus-src/
    Cargo.toml
    src/                           # Rust 构建辅助 API
    vendor/libmodbus/               # 固定上游源码与许可证
    patches/                       # 仅有真实补丁时增加，逐项记录
    examples/build-shared.rs        # 显式动态库构建入口
  libmodbus-sys/
    Cargo.toml
    build.rs
    src/                           # 原始绑定
    native/                        # 必要的通用 C 辅助函数
  libmodbus-rs/
    Cargo.toml
    src/                           # 安全 API 与 RAII
    tests/
```

三个包逐项加入根 `members`；包名保持用户确认的名称。`src` 是构建依赖，不是 runtime 引擎；`rs → sys`，`sys --vendored(build-dependency)→ src`。本轮私有使用，不冒用现有同名公开 crate 的身份；公开发布时另行检查名称与版本策略。不采用独立嵌套 workspace，避免重复锁文件和模式漂移。

### 2. 固定源码与构建辅助包

基线为官方标签 `v3.2.0`，对应提交 `a9b025d12289855490b10d77461c99e001abfc0f`。已有记录中的归档 SHA-256 为 `0c61007b2815daf452618f8b877d15cdcd376b71ad5dbd06b330d70f53b2ccaa`；实施时重新下载并实际计算匹配后才导入源码，不能把旧公式校验和当作已验文件。

选择随包保存源码而非 Cargo 构建期间 git clone；保留许可证、作者、版本来源和补丁清单。源码只读，配置/生成头文件/编译产物进入目标及模式隔离的输出目录。相同 target 的静态、动态、debug/release 不共用可变构建树；缓存键至少涵盖源码/补丁、目标、模式、profile、相关编译设置。

优先沿用上游 Autotools/Libtool（Unix）。源码快照若缺 configure，先在独立构建副本运行 autogen.sh；明确 C/C++ 编译器、make、autoconf、automake、libtool 要求，不把 Windows 或 CMake 构建方式凭空等同于 Unix。Windows 按上游配置生成/Visual Studio 路径评估 MSVC ABI、导出/import library 和 CRT。无法执行的 target 明确报错或标未验证，不误用 HOST 产物。

`libmodbus-src` 提供 Rust `Build/Artifacts` 风格接口，选择 static/shared、target、输出前缀，返回 include/lib/runtime-library 路径和构建清单；无 ESSim 参数。通过 build-shared 示例显式生成动态库安装前缀，供非 vendored 模式使用；该入口先覆盖本机 target，未实现交叉构建时明确拒绝。

### 3. feature 和显式路径契约

| 入口 | 配置 | 行为 |
| --- | --- | --- |
| `libmodbus-rs` 默认 | 默认 feature `vendored` | 转发 sys/vendored，编译内置源码并静态链接 |
| `libmodbus-sys` 默认 | 无默认 feature | 需要显式动态库前缀；不自动查找系统 |
| `libmodbus-sys` + vendored | 可选构建依赖 src | 只静态链接源码产物 |
| `rs` 关闭默认 feature | `LIBMODBUS_DIR=<自行编译前缀>` | 使用该前缀的动态库与匹配头文件 |
| 同时 vendored 与显式动态目录 | 冲突 | 明确报错，要求去掉其中一项 |
| 无 vendored 且目录缺失/不完整 | 无可用来源 | 明确报错，提示构建动态库或开启 vendored |

不增加 `static`、`dynamic` 或 NO_VENDOR feature/环境变量，避免与 vendored 语义重叠。不采用 OpenSSL 的自动宿主探测，只借鉴它的分层和 vendored 含义。

所有通过 workspace 继承的 sys/rs/基础设施依赖在公共声明处关闭默认 feature，实际使用入口显式转发 vendored；否则下游 `--no-default-features` 无法关闭被依赖项隐式开启的 feature。基础设施和 Tauri 应用默认都开启 vendored，并各自向下转发。测试分别执行 feature 图检查和分离构建；Cargo feature 合并后开启 vendored 就是静态，不能以环境变量偷偷切换。

`LIBMODBUS_DIR` 是 include/modbus、lib 的绝对前缀，来自 build-shared 的构建产物；sys 校验头文件/链接库、版本/target/构建清单，动态链接不只看同版本系统库。目录无效、架构错误或库版本不符均失败，不能回退。首轮不继续接受旧 `LIBMODBUS_PREFIX`；若仍设置旧变量，迁移诊断必须明确提示，不静默解释为新模式。

### 4. sys 只提供 FFI 与链接机制

设置 `links = "modbus"`，保证依赖图只有一个原生链接所有者。sys 负责选择来源、编译必要的通用 C 辅助函数，向 Cargo 输出模式、头文件、库目录、版本及动态产物位置等构建元数据；静态场景明确链接原生必要的系统依赖。

先覆盖现有 TCP 服务端用到的官方 API 子集。绑定必须逐项核对固定头文件，以 opaque pointer 表示 C 对象；mapping 布局如保留在 C，辅助函数只能做通用的寄存器访问/边界检查。用 C 编译/链接探针和 ABI/边界测试验证，不照搬猜测布局。此小范围优先最小手工声明+探针，避免首轮额外要求使用者安装 libclang；未来扩大 API 再评估 bindgen。

sys 不分配 SOC、不编码功能策略、不管理协议 worker、不负责应用安装包。unsafe 原始函数写清指针、长度、socket、串行访问和释放契约。相同源码的生成头文件必须与被链接产物相匹配。

### 5. rs 提供安全、通用、有限的 API

对当前 context、holding-register mapping、收请求、正常/异常应答提供 Result、RAII 和范围检查。构造中途失败逆序释放资源；设置超时/绑定 socket 的返回值必须检查，异常错误在清理前保存。映射构造检查空区间、地址溢出和上限，运行值仅是任意 UInt16；无内置地址 100、初值 650、SOC 名称或 0–1000 限制。

socket 有唯一所有者，C 借用 fd 不二次 close；允许用 Rust owner 或有生命周期的借用 guard 表达。上下文只在 worker 内创建/使用/销毁，首轮不添加 Send/Sync；无 C 回调。safe reply 不得接受未检查、可截断的任意切片后直接传给 C；使用私有构造的请求类型或充分验证的边界。原始借用不得超过对应对象生命周期。

保留现有“外部 TCP 有界传输→内部 socketpair→libmodbus receive/reply→真实字节→外部发送”路径，但内部传输、帧门禁、只读策略、收发日志和 worker 属于 ESSim 适配器，不包装成 rs 的设备服务。不会仅因重构就新增示意中高层 TcpServer 全套 API。rs 示例使用非 SOC 命名的普通寄存器，并验证其通用写入能力；ESSim 再施加只读策略。

### 6. 动态库编译与应用交付分开

动态模式先调用 src 构建工具形成显式前缀，再通过 LIBMODBUS_DIR 构建 sys/rs/应用。不能因为传了目录就宣布运行时可加载。

- macOS：产物安装名/rpath 及最终 `.app` 布局配合，携带自己编译的 dylib，打包/签名后复验加载路径；不得保留 `/opt/homebrew` 或随机构建目录的运行依赖。
- Linux：计划携带 so 与相对加载路径，并在目标系统验证 libc/架构兼容；本机没有 Linux 测试证据。
- Windows：计划 DLL+匹配 import library，校验目标工具链、SOCKET 位宽与上游已报告问题；静态还须处理头文件 dllimport 宏和 CRT 一致性，未经验证不启用。

应用打包负责携带动态库和许可材料，sys 只提供元数据。本次 macOS 动态验收必须检查加载记录及依赖路径，在不靠全局 DYLD_LIBRARY_PATH/宿主 libmodbus 的情况下运行。静态验收检查不再动态依赖 libmodbus，但不要求移除系统库依赖。

### 7. 许可和验收

保留 LGPL-2.1-or-later 原文与来源。静态默认模式改变发布安排：记录适用条款下所需的库源码、修改和重新链接材料/方案；动态模式同样记录源码提供、许可和库替换安排。具体交付材料在发布前核实，不宣称本提案已完成分发合规；不以动态链接为免责理由。

基线先跑现有测试；再补源码/模式/路径失败测试、sys 探针、rs 生命周期/请求边界/通用寄存器测试。两种模式分别完成既有 TCP 全套及固定 PyModbus 初值 650→确认更新700、异常与重连。Tauri 静态和动态均验证启动、NovaSheet 更新、外部读回、停止及端口释放。原停机目标 <1s 和有界队列/日志保持。最终记录源码提交、工具链、target、链接方式、原始报文和加载路径，未执行平台保留待验收状态。

## Risks / Trade-offs

- [源码构建工具更多、首次构建较慢] → 给出明确前置工具和缓存；不静默改用系统库。
- [Cargo feature 合并导致动态误链静态] → 每层显式转发、统一 default-features=false 和分别构建，检查链接产物。
- [动态库编译成功但打包后无法加载] → 打包路径/rpath/签名验收与库编译分开，检查真实加载位置。
- [Windows API/导出宏/上游 SOCKET 问题] → 单独平台任务，未通过前拒绝，不用强转截断句柄。
- [把教学业务搬进通用库] → 用普通寄存器和可写测试验证 rs，与 ESSim 只读回归分开。
- [静态发布义务未准备] → 源码与许可材料随源码导入记录，发布交付另设检查项，不虚报完成。

## Migration Plan

先记录当前基线；新增三包与源码静态/动态构建验证；将原始 FFI、安全对象、教学策略按边界拆出；接入基础设施及 Tauri feature 传播；两种模式回归后才删除被替代的基础设施 build.rs/C 薄层。保留现有 UI、核心规则、运行端点和 IPC。

旧依赖说明保留历史测试记录，另写新构建说明并标明旧 Homebrew 路径已被新构建替代。失败时只回退本轮库边界/构建修改，恢复原闭环，不动原型和其他协议代码。无数据库迁移。

## Open Questions

核心架构与 feature 语义已确定。实施仍需核实固定源码的工具链要求、macOS 动态库重定位细节及 Windows 3.2.0 上游问题状态；用构建探针解决并记录，不能先声称三平台可用。独立公开发布的 crate 名称、许可证选择与版本策略不属于本轮。

## References

- [上游 v3.2.0](https://github.com/stephane/libmodbus/tree/v3.2.0)、[构建配置](https://github.com/stephane/libmodbus/blob/v3.2.0/configure.ac)、[Windows 说明](https://github.com/stephane/libmodbus/blob/v3.2.0/src/win32/README.md)。
- [上游许可](https://github.com/stephane/libmodbus/blob/v3.2.0/COPYING.LESSER)、[Windows SOCKET 问题 #863](https://github.com/stephane/libmodbus/issues/863)。
- [rust-openssl 构建语义](https://docs.rs/openssl/latest/openssl/#building)、[vendored 调用 src](https://github.com/sfackler/rust-openssl/blob/master/openssl-sys/build/find_vendored.rs)。
- 既有基线：`docs/test-records/2026-10-14-modbus-loop.md`、`docs/test-records/libmodbus-dependency.md`。

## 实施结果与边界（2026-10-15）

已完成三包与两种模式的 macOS 验收，见 `docs/test-records/2026-10-15-libmodbus-workspace.md`。最小原始绑定+官方头文件 C 编译探针替代原私有业务 C 层；基础设施 build.rs 仅保留本地测试/示例的开发 rpath，不再编译 C。动态包由显式脚本拷贝库、移除开发 rpath 并 ad-hoc 签名；不涉及公开签名/公证。Linux GNU 路径未验，Windows/交叉/universal/musl 明确拒绝。上述已验实现无上游补丁，不含完整 API、RTU 或主站；RTU 从站后续已完成下文的软件实施，真实硬件仍待验。

## 补充：四类数据区

新增 AddressRange 与 Mapping，分别配置线圈、离散输入、保持寄存器、输入寄存器的可选零基地址区间；位值使用 bool，寄存器使用 u16。C 辅助函数校验数据区和访问边界，Rust 不解引用 C 布局。保留 HoldingRegisters 兼容入口与 RAII/线程限制。验证 FC01/02/03/04/05/06/0F/10、区间边界、同地址隔离和输入区不被网络写入改变。当前 SOC 示例、IPC 与界面点表不扩展；不宣称新增真实 EMS 或独立工具验收。

补充项已实施，静态/动态封装与原 SOC TCP 回归通过；详见 `docs/test-records/2026-10-15-libmodbus-four-areas.md`，不将旧桌面和独立客户端记录算作新增四区验收。

四区命名统一为 `holding`（保持寄存器）、`input`（输入寄存器）、`coil`（线圈）、`discrete`（离散输入）；Mapping 的本地访问方法使用这些名称及 `set_` 前缀。构造参数顺序沿用 C 映射顺序 coil、discrete、holding、input，不因显示顺序调整。上游 C API 和 `HoldingRegisters` 兼容入口保留原名。

## 补充：动态 Unit ID

同一 TCP 端点下可在停止或监听状态新增 Unit ID，本阶段产品范围为 1–247。默认保留单元 1，每个单元拥有独立 SOC 初值 650 和独立 libmodbus context/mapping，由同一 worker 串行管理。按请求 Unit ID 路由到对应映射；未知 ID 沿用关闭连接行为。新增及更新通过命令确认，映射创建成功后才发布完整快照；重复/非法 ID 拒绝。停止后再启动保留本会话配置和值，不提供删除、磁盘持久化或规模承诺。

界面提供 Unit ID 下拉框和数字输入框旁的添加按钮，新增成功后选中新单元；不同单元保留独立草稿，日志快照不覆盖草稿。IPC 和统一客户端支持 addUnit 与携带 unitId 的 updateSoc；旧更新入口默认作用于 Unit ID 1。仿真核心仍不依赖 Tauri，libmodbus-rs 不承担单元集合及设备调度。

## 补充设计：RTU 串口从站（软件已实现，硬件待验）

### 配置与角色

新增 `RtuConfig`，表达设备路径、波特率、parity、data bits、stop bits、从站号和有限接收/字节间超时。设备路径校验空值和内嵌 NUL，波特率须为正且受目标平台支持；校验位 N/E/O、数据位与停止位的参数组合以固定上游 API、RTU 规范及设备资料核实后形成支持清单，不能因 C 构造接受参数就宣称协议与硬件支持。配置失败须保留明确错误，不静默改用其他串口或参数。

首阶段只运行一个串口和一个从站，站号 1–247；0 仅作为接收侧广播地址，不可配置为本机站号。TCP/RTU 角色和通信配置分别建模；不把 TCP Unit ID 集合自动映射为同串口多个 context，不让多个 reader 争抢同一串口。

### 安全封装与资源所有权

`RtuContext` 在所属 worker 中创建及使用，不实现 Send/Sync，无 C 回调。通过官方 `modbus_new_rtu` 创建 context，设置站号和超时，再由 `modbus_connect` 打开串口。串口描述符由 libmodbus 独占管理，Rust 不再用 OwnedFd/TcpContext 的借用路径关闭它。RAII 记录已取得资源，打开后的正常退出或失败均先 `modbus_close` 再 `modbus_free`；尚未打开的失败路径只释放 context，保存清理前的原始错误。核实 connect 内部部分失败的清理行为并用失败注入验证，不能重复 close。

RTU 复用现有 `Mapping`，不将 SOC 放入通用库。采用独立 `RtuRequest`防止 TCP ADU 被传给 RTU reply，反之亦然；已用跨协议混用的编译失败测试约束 API。RTU 最大 ADU 为固定头文件中的 256，当前 TCP 上限为 260，分别命名，不能沿用 TCP 请求的最短长度和 MBAP 索引。只有成功接收、完成对应请求结构校验的帧才能构造可应答请求；receive 返回 0 表示被忽略，与错误及正常请求区分。

### 收发、广播与停机

RTU 直接通过串口 receive/reply；CRC、帧接收交给上游库，不手写产品 CRC/协议栈，不经 TCP frame_length 或 Unix socketpair。区分接收等待、字节间超时、CRC/结构错误、非本站帧和设备断开；完整、分段、连续请求及恢复行为均要实测，不把 libmodbus 字节数接收机制描述成完整 RTU 时序符合性保证。

广播在正常和异常应答入口都不得发送响应。固定源码的 `modbus_reply` 有广播抑制，但 `modbus_reply_exception` 直接发送，因此必须在安全封装的异常入口显式保护，不能只依赖正常 reply。通用 Mapping 可按协议执行合法广播写入；ESSim 当前 SOC 对外只读，须在调用会修改映射的正常 reply 前拒绝写入，对广播静默忽略且不改 SOC。

worker 串行处理命令和串口 I/O，配置有限超时，在停止时退出收发再关闭串口、释放 context/mapping 并 join；前端不得跨线程直接 close C context。不能假定接收超时也限制串口写入、驱动 drain 或拔出行为，需核查这些阻塞路径并设计有界处理。以空闲、半帧、持续无效流量、串口断开及发送受阻测试给出停机上限和支持条件；目标为已验参数/平台停机 <1s，无法达到时记录原因并调整方案，不提前宣称通过。

日志仅记录实际收到的帧、实际错误和应答结果；串口 reply 直接发送时若未捕获真实 TX 字节，只记发送长度/事件，原始 TX 验证使用独立串口端抓取，不重建报文冒充实际发送。

### 工作台与状态

保持 Dockview 紧凑工作台；通信栏增加 TCP/RTU 选择，TCP 显示现有 IP/端口和多 Unit ID 控件，RTU 显示串口参数及单站号，首版手动填写设备路径。通信方式及参数仅在停止状态应用，配置整体校验后生效，不留半应用状态。RTU 不显示 TCP 连接数语义，也不允许动态添加多个同串口站号；切换配置保留各自草稿及稳定标识，不把 TCP 多单元值误应用到 RTU。

统一客户端、核心配置和快照使用可区分 TCP/RTU 的通信类型，Tauri 仅做适配，保留原 TCP 调用兼容性；无界面 RTU 入口调用同一应用服务。RTU 沿用教学 SOC 地址/倍率、内部确认更新和外部只读策略。

### 验证与平台边界

先写参数、资源、请求类型和协议失败用例，再实施。以 PTY/虚拟串口及独立 Modbus RTU 客户端验证四区通用读写、站号过滤、CRC/截断/连续帧、超时恢复、广播与生命周期；独立客户端版本在实施时锁定并保存完整收发、预期与实际结果。静态/动态分别验证 RTU 及既有 TCP 回归，新增支持不改变已有平台拒绝策略。

真实串口/RS-485 另行验证适配器、驱动、两端参数、接线、方向控制和时序，记录硬件与测量方式；PTY 不证明电气或总线时序。无硬件时保留未完成任务。Linux/Windows 未实测不宣称支持，当前 Windows 构建拒绝不因加入 RTU 被绕过。RTU 从站首阶段不加入自定义 RTS 回调、广播响应 quirk 或多站调度；后续通用主站子集见下文。

依据为内置 v3.2.0 的 `docs/modbus_new_rtu.md`、`modbus_connect.md`、`modbus_set_slave.md`、`modbus_close.md`、`src/modbus-rtu.h` 及 `src/modbus.c`；这些本地官方资料用于计划，不能代替运行验收。

### 本轮实现选择与验证结果

已实现 RtuConfig/RtuContext/RtuRequest、官方 FFI 探针、独立 worker、无界面入口、带 transport 的配置/快照与工作台参数控制。首版参数为 9600/19200/38400/57600/115200、8 数据位、N/E/O、1/2 停止位，请求等待 10–100ms、字节间等待 1–2ms；这些限制用于约束最长接收等待，不表示每个组合硬件通过。直接使用上游 O_NONBLOCK、RTS_NONE、不启用链路重试的路径，发送受阻报错。清理以实际打开标志决定 close，错误先保存为 Rust Error 再 Drop。

静态/动态各 31 个 Rust 测试和 8 个文档测试、11 个前端测试、两模式独立 RTU 无界面与桌面 650→700 闭环通过；真实串口/RS-485 任务 9.9 保持未完成。具体实测参数、失败定位、原始报文和开发动态库路径边界见 `docs/test-records/2026-10-15-rtu.md`。不执行归档。


## TCP / RTU 主站补充设计

新增 ModbusClient，内部独占 TcpContext<TcpStream> 或 RtuContext，不公开原始句柄，不实现 Send/Sync。TCP 使用数值 SocketAddr 和 std::net::TcpStream::connect_timeout，分别配置连接等待、socket 写等待和 C 响应等待；C 借用 Rust socket，释放规则不变。RTU 复用已校验串口配置，由 C 管理串口；主站响应等待独立配置，不采用从站 10–100ms 的等待限制。不启用 RTS 或广播，目标 TCP 0–255、RTU 1–247。

直接调用上游 FC01/02/03/04/05/06/0F/10，不自行实现协议编解码。读取数量分别最多 2000 位/125 寄存器，多写最多 1968 位/123 寄存器；所有操作检查 16 位地址区间。bool 转换为逐点 0/1 缓冲交给 C 打包。精确校验成功返回数量。io::Error 保留上游 errno，exception_code 提取 1–11 异常。无自动重试；非合法异常错误立即释放通道，后续操作返回 NotConnected。

response_timeout/byte_timeout=0 沿用上游等待语义，select 在平台间对 timeval 的处理不同，不能承诺墙钟整次请求截止时间。固定 3.2.0 TCP pre_check_confirmation 检查事务号与协议号，但不严格校验响应 Unit ID 或 MBAP 长度；不将该通用客户端宣称为不可信协议防火墙。TCP/PTY 测试覆盖已检测错误，独立 PyModbus 服务端验证普通互操作，真实设备和物理串口另验。
