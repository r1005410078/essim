# lib60870 Rust 三包

三包位于根 Cargo workspace，共用 Cargo.lock，不依赖 ESSim core、Tauri 或 Tokio，不创建嵌套 workspace。

| 包 | 职责 |
| --- | --- |
| `lib60870-src` | 完整固定官方源码、离线 CMake 构建、静态／动态私有产物与来源清单 |
| `lib60870-sys` | 唯一 `links="60870"` 所有者、原始 FFI 子集、C 头文件 ABI 探针与链接元数据 |
| `lib60870-rs` | 线程绑定的 ASDU／信息对象和首批 104 服务端安全 API |

依赖方向为 `rs → sys`，sys 的构建脚本使用 src。不会在编译时联网下载，也不使用 pkg-config、Homebrew 或系统已有协议库。

## 源码与许可

官方 lib60870-C 标签 **v2.4.1**，固定提交 `7a388e3e133999e1ca77ba7521d55d074b7cd2bc`；[来源及摘要](lib60870-src/SOURCE.md)。保留完整 101／104 源码、示例与文档，未修改 vendor。104 复用 `cs101_asdu` 和信息对象，不按文件名删除 101 部分。

上游该标签的版本宏与运行时查询仍返回 **2.4.0**，CMake 则是 2.4.1。构建器校验来源标签／提交／源码树，ABI 测试另核实实际头文件和运行时，未自行修改上游宏。

上游提供 GPLv3／商业双许可，原文保留在 `vendor/lib60870/COPYING`。这与 libmodbus 的 LGPL 条款不同；本轮只建立私有 `publish=false` 包，不决定 ESSim 整体发布许可，不表示已经获得商业授权或完成分发安排。

## 首批安全 API

- `Asdu`：标准 104 应用参数（TypeID/VSQ/COT/CA/IOA 字节数为 1/1/2/2/3，OA=0，ASDU 上限249字节）、CA、传送原因、连续／独立 IOA。
- `Point::single`：M_SP_NA_1；`Point::measured`：M_ME_NC_1。地址、质量保留位、单点 OV、非有限浮点、混合类型、连续地址、对象数及编码容量均检查。
- `Server::bind`：无线程 104 服务端，应用显式 `tick`，固定单冗余组、最多10个客户端、高／低优先级队列参数各64。上游高队列按编码字节容量管理，不能把参数64当作所有ASDU长度下的精确条目上限。
- `Request`：回调作用域内的类型／地址／原因／支持点值／总召 QOI 读取，以及 ACT_CON、发送、ACT_TERM。回复成功仅代表库已接受发送或排队。
- `Server::enqueue`：周期／自发 ASDU 上送，满64条时返回 `WouldBlock`，避免上游默认覆盖旧事件。它不提供持久化、应用级交付保证或自动重试。

不是完整 lib60870 API。尚未封装 101 串口通信、104 客户端、带时标点、其他监控／控制类型、TLS、安全认证、文件服务、多冗余组、原始报文订阅。上游有这些代码不代表安全层已支持。

## G0监控实验接口（默认关闭）

`g0-monitor`仅供协议重构的原生队列可行性验证，工作台及生产适配器未接入。它提供无裸指针、不可跨线程的SessionToken、回调内复制的受限站总召Interrogation（CA／OA／测试标志及响应构造），以及所有者线程的 `active_sessions`／`session_is_current`／`try_send_to`。总召数据、确认和终止均可在回调返回后构造，Request／C借用不逃逸，不提供延迟控制功能。

发送只报告库接纳；队列拒绝返回WouldBlock，由调用方保留FIFO队首并推进tick重试。STOPDT、断连、停止及重复STARTDT均使旧token失效；内部接收通知按固定上游实际重置分支识别，包括其接受的非标准控制帧，保留TESTFR优先级，不将其宣称为合法104帧。session_is_current不发送ASDU，调用方应在每次tick后查询，先替代旧应用积压并捕获当前快照，再接续新变化，即使FIFO为空也能检测重置。查询只反映已观测状态，不排除尚未观测的断网。不开启原始报文订阅或Sent／Acked保证。

单FIFO只约束入队后的顺序。应用仍须按完整ChangeBatch的epoch／after／through范围整批接纳，快照cut不得落后于已接纳高水位；乱序、重叠、断档或旧快照进入Gap并重取权威快照；当前epoch／位置的空轮询不触发重同步。不能将同一提交产生的多帧拆成重复游标的独立更新。策略不混用Server.enqueue低队列，也不承诺历史SOE／离线无损。具体调度与恢复目前仅由G0测试设施验证，未接入生产。

```sh
cargo test -p lib60870-rs --features g0-monitor --test g0_queue --offline -- --nocapture
```

实际TCP积压、100帧总召、迟到旧投影替代、背压及会话失效证据见[G0记录](../../docs/test-records/2026-10-25-protocol-g0.md)。这不是正式104模拟器验收。

## 线程、回调与资源

ASDU 的参数保存在稳定地址的 Box 中，先销毁 C ASDU 再释放参数；信息对象临时分配，编码复制进 ASDU 后独立释放。`Asdu` 和 `Server` 均为 `!Send/!Sync`，没有 `unsafe impl Send`。回调 ASDU／连接仅在该次回调内借用，安全接口不暴露裸指针，解码点值为独立 Rust 数据。

`tick` 在创建线程同步执行回调；处理器应快速完成，不做无界 I/O、不重入 server。Rust panic 被 `catch_unwind` 捕获，当前 tick 结束后停止服务并报告错误；为了避免恶意 panic payload 的析构再次 unwind，故障路径保留该 payload。`panic=abort` 与进程级 OOM 无法由该机制恢复。

Drop 先 stopThreadless 关闭监听及连接，再 destroy C server，最后释放回调状态。服务器不隐式创建协议线程；示例单独读取 stdin 的线程不持有 C 句柄。无线程模式不代表 tick 严格不阻塞，不承诺实时性或规模性能。

协议业务仍由使用者定义：CA／IOA 映射、总召范围、控制拒绝／执行及应用状态。不要将回调观察日志当作 Modbus 现有“可靠落盘确认后才发送”的屏障；该语义尚未接入。

## 静态构建与验证

需要 Rust/Cargo、C/C++ 编译器、CMake 和其默认构建工具（本机 Unix Makefiles 使用 make）。上游来源完整保存在仓内；所有构建修改均发生在输出目录副本，禁止写入 vendor 或系统前缀。

```sh
cargo test -p lib60870-src -p lib60870-sys -p lib60870-rs --offline
cargo clippy -p lib60870-src -p lib60870-sys -p lib60870-rs --all-targets --offline -- -D warnings
cargo doc -p lib60870-src -p lib60870-sys -p lib60870-rs --no-deps --offline
cargo build -p lib60870-rs --example iec104-server --offline
python3 scripts/verify_iec104_wrapper.py --binary target/debug/examples/iec104-server
```

`rs` 默认启用 vendored 静态构建；单独使用 sys 时必须显式 `--features vendored` 或给动态前缀。`vendored` 与 `LIB60870_DIR` 同时存在明确报错。关闭 vendored 却没有有效前缀也报错；前缀必须绝对路径，并通过来源／目标／头文件／原生库摘要校验。

示例地址 CA=47、IOA=100、浮点12.5均为教学约定。示例仅接受该 CA 下 QOI=20、COT=6 的总召，其他请求的业务处理不能从此推断；输入 `quit` 或 EOF 后正常关闭。

## Examples：一键验证安全封装

在仓库根目录执行，无需启动工作台或另外安装客户端：

```sh
cargo run -p lib60870-rs --example iec104-smoke --offline
```

[示例源码](lib60870-rs/examples/smoke.rs)直接调用安全封装启动回环服务端，独立测试线程发送原始 TCP 报文并逐字节核对响应。服务端与 C 句柄始终留在创建线程，不使用 Tokio。

覆盖 STARTDT、分段总召、ACT_CON／ACT_TERM、单点与短浮点遥测、自发上送、不支持的分组总召负确认、TESTFR、STOPDT，以及关闭后端口重绑。每一步输出 `PASS`；出错或超时以非零退出码结束并清理服务端和客户端线程。示例有5秒整体驱动期限和1秒网络读写超时。

CA=47、遥测 IOA=100（12.5→14.0）、单点 IOA=200（true→false）均为教学测试约定。原始字节客户端仅覆盖这些测试路径，不是安全层新增的通用104客户端，也不能代替独立协议栈或真实 EMS 互操作验收。

## 显式动态构建

先生成私有动态前缀，命令最后打印绝对路径：

```sh
cargo run -p lib60870-src --example build-60870-shared --offline -- target/lib60870-shared
```

将实际打印路径赋给 `LIB60870_DIR` 后执行（以下占位符需要替换）：

```sh
LIB60870_DIR=/absolute/generated/install CARGO_TARGET_DIR=target/lib60870-dynamic \
  cargo test -p lib60870-sys -p lib60870-rs --no-default-features --offline
LIB60870_DIR=/absolute/generated/install CARGO_TARGET_DIR=target/lib60870-dynamic \
  cargo run -p lib60870-rs --example iec104-smoke --no-default-features --offline
```

本地 rs 测试与示例携带显式前缀 RUNPATH；下游其他包如直接使用动态库，须自行设置加载路径。该开发路径不应直接进入应用发布包。没有改动现有 Tauri 打包脚本，也没有把 104 链接到工作台。

## 验证边界

本轮实际验证 macOS arm64：静态／动态、C ABI、ASDU、真实回环 TCP 总召、回调 panic、队列拒绝、资源释放和文档示例。Python 是独立原始字节客户端，覆盖 STARTDT、分段总召、响应载荷、合并 TESTFR／STOPDT 和 quit 后重绑；不是独立成熟协议栈互操作或完整标准符合性认证。

Linux GNU 原生构建路径尚未实测。Windows、交叉编译、musl、universal 当前明确拒绝。真实 EMS、101 串口、硬件、TLS 和规模未验收。详细结果见[实施记录](../../docs/test-records/2026-10-25-lib60870-packages.md)。
