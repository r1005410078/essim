## Why

ESSim 正式工程目前只有工作台与核心信息查询，尚不能与真实 EMS 通信。需要以成熟 libmodbus 库完成一个可独立验证的 TCP 从站闭环，验证 Rust 安全封装、运行状态所有权及前端通信边界。

## What Changes

- 在基础设施层接入固定版本的 libmodbus，集中封装 FFI、句柄、映射及套接字释放，不在产品路径重写协议栈。
- 在核心建立单实例启停、运行值更新和状态查询接口；协议线程独占 C 资源，通过消息与核心协调。
- 提供固定教学 SOC 点：保持寄存器零起始地址 100、UInt16、倍率 0.1、初始原始值 650，即 65.0%。此为教学约定，不是设备标准地址。
- 支持 FC03 读取；界面修改 SOC 后，外部客户端下一次请求读取新值。SOC 禁止外部写入，明确拒绝越界地址与未支持功能。
- 接入正式工作台的最小启停、SOC 编辑与真实运行状态，业务组件通过统一客户端接口操作。
- 增加无界面验证入口、TCP 分段/合并及非法报文用例，并记录独立客户端互操作证据。
- 本次不接入 RTU、其他协议、多设备规模、通用点表导入、SQLite、策略或独立 Web 服务。

## Capabilities

### New Capabilities

- `modbus-tcp-simulator`: 单 TCP 从站的生命周期、教学点读取、运行值修改、拒绝行为及故障恢复。
- `libmodbus-safe-adapter`: C 库版本与构建、Rust 安全资源封装、协议线程所有权、可中断停机及互操作验证。
- `modbus-workbench-bridge`: 不绑定 Tauri 的前端业务接口、Tauri 操作与运行快照，以及最小闭环工作台。

### Modified Capabilities

无。当前没有已发布 OpenSpec 规格。

## Impact

- `crates/core`：领域状态、应用用例和协议执行端口。
- `crates/infrastructure`：libmodbus 构建接入、安全封装、TCP 适配器、无界面示例及集成测试。
- `workbench/src-tauri`：实例管理、命令、运行订阅与统一 DTO。
- `workbench/src/shared/api` 与业务页面：类型契约、Tauri 适配与闭环操作。
- 增加 C 构建/运行依赖和许可证记录；当前机器未找到 `pkg-config`，实施前需建立可复现依赖路径。
- README、架构与学习记录更新；`prototype/` 不修改，不创建 Qt 或远程服务工程。
