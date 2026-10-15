# TCP / RTU 主站软件验收（2026-10-15）

## 范围与环境

本次增加 `libmodbus-rs::ModbusClient`，不增加工作台主站 UI、采集轮询或设备业务。
环境为 macOS 26.4.1 / arm64，Rust 1.96.0，aarch64-apple-darwin；上游固定 libmodbus 3.2.0，vendored 源码未修改。
验证默认源码静态和显式自行编译动态两种模式。独立从站为 PyModbus 3.11.3，RTU 使用 pyserial 3.5 和两对 Unix PTY 转发。
没有真实设备、EMS、USB 串口或 RS-485 电气/方向/时序验证，也没有 Linux/Windows 验收或规模/性能承诺。

## 实现与恢复语义

- TCP 使用数值 SocketAddr 主动连接，连接等待、socket 写等待和 C response_timeout 分别设置；C 借用 Rust socket，Rust 唯一关闭。
- RTU 复用已校验串口配置，由 C 打开并管理串口，close → free；独立设置主站 response_timeout，禁用 byte_timeout。
- 支持 FC01/02/03/04/05/06/0F/10：四区读取、线圈与保持寄存器单/多写，bool/u16 原始值，不做倍率/字序换算。
- 数量和 0–65535 地址区间发送前校验：读位最多2000、读寄存器125、多写位1968、多写寄存器123。
- set_unit 切换后续目标，TCP 0–255，RTU 1–247，不支持广播。一个对象一次一个请求，不实现 Send/Sync。
- io::Error 保留 errno，exception_code 提取1–11异常码；合法异常后允许继续请求。
- 超时、I/O 或上游检测到的无效确认立即释放通道，后续返回 NotConnected，须重新构造。无自动重连/重试；写入失败可能已经执行。RTU 重开不保证迟到响应已消失，恢复需协调总线。

固定上游 `src/modbus-tcp.c::_modbus_tcp_pre_check_confirmation` 检查事务号和协议号，不严格检查响应 Unit ID 或 MBAP 长度；本封装沿用该边界，不作为不可信从站协议防火墙。上游 `src/modbus.c::_modbus_receive_msg` 的 response_timeout 和 select/timeval 依赖平台行为，本次不承诺整次操作墙钟截止时间，也未把两次分段 write 当作必然发生的网络分包。

## 失败用例与 Rust 回归

先增加依赖尚不存在的 ModbusClient/exception_code 的失败用例，确认编译失败，再实现接口。新增6项集成测试覆盖：

1. 空/超数量、地址溢出在发送前拒绝；远端异常码02保留，下一次合法读取正常。
2. 写入超时断开且没有第二次自动请求，后续操作返回 NotConnected。
3. PTY 四区读取、四种写入、逐字节预期请求与CRC、站号7切到8、拒绝0/248。
4. CRC错误响应拒绝并关闭通道。
5. TCP事务号不匹配、功能码不匹配、截断/断连响应拒绝并关闭通道。
6. 无效超时在连接/打开前拒绝；RTU无响应超时关闭通道并可显式重新打开。

既有从站映射、SOC只读策略、多Unit、超时停机和资源失败清理回归均通过。两种链接模式各 **37项 Rust 单元/集成测试 +9项文档测试**通过，包括新增主站不允许跨线程的编译失败示例。

```sh
cargo test --workspace --offline
LIBMODBUS_DIR="$PWD/target/libmodbus-shared/aarch64-apple-darwin-Dynamic-debug/install" cargo test --workspace --no-default-features --offline
cargo doc -p libmodbus-rs -p libmodbus-sys --no-deps --offline
```

触及 Rust 文件的 rustfmt 检查通过，Python脚本语法检查通过，OpenSpec严格校验通过。未改前端，本次未重复前端/UI验收。

## 独立从站互操作

`client_demo` 是会写入教学点的验收示例，不用于生产设备。脚本 `scripts/verify_modbus_master.py` 每次自动启动独立 TCP 和 PTY RTU 从站。
教学参数：站号7、零基地址100–101；coil=[true,false]、discrete=[false,true]、holding=[42,43]、input=[99,100]；RTU为19200/N/8/2。
PyModbus 3.11.3 的 DeviceContext 内部地址 +1，因此数据块起点为101，线上仍为100，脚本已注明。

每种传输每种链接模式执行14次请求：四区读取、单写coil=false与holding=12并读回、多写coil=[true,true]与holding=[55,56]并读回、地址500得到异常02、异常后读取input仍为[99,100]。两种模式均输出两次PASS，共56次请求/响应。

```sh
cargo build -p libmodbus-rs --example client_demo --offline
/private/tmp/essim-pymodbus-314/bin/python scripts/verify_modbus_master.py
LIBMODBUS_DIR="$PWD/target/libmodbus-shared/aarch64-apple-darwin-Dynamic-debug/install" cargo build -p libmodbus-rs --example client_demo --no-default-features --offline
/private/tmp/essim-pymodbus-314/bin/python scripts/verify_modbus_master.py
```

上述Python路径为本次验收环境，复现时替换为安装了固定版本依赖的解释器。动态示例 `otool -L` 显示 `@rpath/libmodbus.5.dylib`，本机显式开发前缀可运行；不代表主站示例已完成独立公开分发打包。

完整原始报文：[静态模式](2026-10-15-master-static.log)、[动态模式](2026-10-15-master-dynamic.log)。SERVER RX 是从站实际收到的请求，SERVER TX 是独立从站输出的响应。关键RTU报文：

```text
RX 07 03 00 64 00 02 85 B2                 # 读取holding100–101
TX 07 03 04 00 2A 00 2B FD E4              # [42,43]
RX 07 10 00 64 00 02 04 00 37 00 38 5A E0 # 多写[55,56]
TX 07 10 00 64 00 02 00 71
RX 07 03 00 64 00 02 85 B2
TX 07 03 04 00 37 00 38 2C 2F              # 读回[55,56]
RX 07 03 01 F4 00 01 C4 62                 # 教学无效地址500
TX 07 83 02 20 F0
```

独立普通读写互通不等于完整 Modbus 兼容性证明。原提案任务9.9真实RS-485硬件验证继续未完成。
