## Why

ESSim 下一步需要接入 IEC 104，先建立与现有 libmodbus 相同职责分层的独立 Rust 协议库。复用官方 lib60870-C，不在产品路径重写协议栈。

## What Changes

- 固定官方 lib60870-C v2.4.1 提交，完整保留 101／104 源码与许可。
- 新建 lib60870-src、lib60870-sys、lib60870-rs 三包并加入根 Cargo workspace。
- 提供离线静态构建、显式动态来源、原始 FFI 与首批 104 安全 API，保持核心、Tauri 和 Tokio 无依赖。
- 通过 ASDU、真实 TCP 与资源释放测试验证封装；记录支持子集、平台与未验范围。
- 用户追加独立 `iec104-smoke` Examples：一条 cargo run 命令用安全封装启动服务端，以字节级客户端验证总召、遥信／遥测、主动上送、负确认及退出。

## Capabilities

### New Capabilities

- `lib60870-rust-packages`: 可追溯源码构建、FFI 与线程／生命周期安全的首批 104 API。

### Modified Capabilities

无。

## Impact

新增 crates/lib60870 三包、根 Cargo 配置与锁文件、库说明和实施记录。不接入工作台、不修改 Modbus 执行器、不提供 101 安全通信 API、TLS 或完整 104 兼容性承诺。
