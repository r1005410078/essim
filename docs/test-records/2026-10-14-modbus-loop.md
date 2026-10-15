# 单 SOC Modbus TCP 闭环验收

实施始于 2026-10-14，NovaSheet 桌面复验于 2026-10-15 完成。

## 环境与约定

macOS 26.4.1 / arm64；libmodbus 3.2.0 动态库；Tauri 2.12.3、React 19.3.0、PyModbus 3.11.3（临时 Python 3.14.4 环境）。依赖来源与平台限制见 [依赖记录](libmodbus-dependency.md)。本次是本机回环、1 个实例、最多 1 个活跃客户端、1 个点，不是规模测试或真实 EMS 验证。

教学映射：`demo-bms/soc`，`127.0.0.1:1502`，Unit ID 1，FC03 保持寄存器零起始地址 100，UInt16 单寄存器。初值 650，×0.1%=65.0%；范围为原始整数 0–1000。仅内部应用可修改运行值，外部写入拒绝。重启监听保留会话运行值，新进程重新初始化为 650。

## 独立互操作

使用 `scripts/verify_modbus.py`，固定 PyModbus 3.11.3，不使用 libmodbus 客户端。依次验证：

1. Tauri 桌面启动后，外部读到 `[650]`。
2. 工作台更新为 700，确认后外部读到 `[700]`。
3. 地址 101 读返回异常 02；FC06/FC16 写返回异常 02；FC04 返回异常 01。拒绝写入后 SOC 不改变。
4. 客户端关闭并重新连接，仍可读取正确值。
5. 桌面点击停止后，另一个 socket 成功 bind 1502，证实监听已释放。

原始收发记录：[初值 650](2026-10-14-pymodbus-650.txt)、[更新 700](2026-10-14-pymodbus-700.txt)。响应中的 `02 8A`=650，`02 BC`=700；事务号由客户端生成并回显。

2026-10-15 在真正的 macOS Tauri 生产前端包中复验 NovaSheet：序号列显示；双击草稿输入 700、按 Enter 后，外部仍读 650；点击工具栏 ✓ 后外部读 700；双击已生效值列不会进入编辑。证据：[未应用草稿读回](2026-10-15-novasheet-draft-650.txt)、[应用后读回](2026-10-15-novasheet-applied-700.txt)、[桌面截图](2026-10-15-novasheet-desktop.png)。组件 vendor 的 JSX 开发接口与 React 只读属性转发问题由工作台兼容层处理，没有修改 NovaSheet 项目。

无界面示例也实测：端口 1503 启动，独立客户端读回 650；stdin 输入 700 返回 ACK，再读回 700；输入 quit 输出 STOPPED。使用同一个 SimulatorService / ModbusSimulator，无 Tauri 窗口依赖。

## 专项测试

Rust 测试覆盖 SOC 非整数/越界/NaN、稳定点 ID、端点校验、初始值隔离、重复启动、停止幂等、端口占用失败与恢复、重启保留值、只读策略、数量非法、Unit ID、分段/合并、非法 MBAP、截断 PDU、额外连接拒绝与连接恢复。停机断言覆盖空闲监听、客户端、半帧及不读取响应的客户端，要求 <1s；是功能时限检查，不代表延迟基准。接收积压最多 1040 字节，超过会关闭客户端，避免无界排队。

C 初始化失败注入在 context 后和 mapping 后各循环 100 次，并检查借用 socket 仍可使用；正常创建/销毁循环 100 次。未运行 ASan 或独立内存泄漏检测。

前端测试覆盖无后端明确失败、IPC 稳定 ID、取消订阅幂等、旧代次/旧序号过滤与大整数精度、NovaSheet 草稿的整数和范围校验。订阅队列最多 64 个快照，日志最多 128 条；桥接会查询最新完整快照，补齐队列满时合并的最后状态。

## 复现

先按依赖记录准备 libmodbus 3.2.0。

```sh
pnpm install
pnpm desktop
# 工作台启动 127.0.0.1:1502
python3 -m venv /tmp/essim-interop
/tmp/essim-interop/bin/pip install pymodbus==3.11.3
/tmp/essim-interop/bin/python scripts/verify_modbus.py --expected 650
# 在 NovaSheet 中编辑草稿 700、Enter、点击 ✓
/tmp/essim-interop/bin/python scripts/verify_modbus.py --expected 700
```

Python 需 ≥3.10；本机系统 Python 3.9 无法安装该固定版本，因此实际使用 Homebrew Python 3.14.4。

最终回归结果：Rust 9 个测试、前端 5 个测试通过，类型检查、格式检查、生产构建及 OpenSpec strict 校验通过。

相关检查：`cargo test --workspace --offline`、`cargo fmt --all -- --check`、`pnpm typecheck`、`pnpm test`、`pnpm build`；并构建/打开了 debug Tauri `.app`，其前端使用生产构建。`.app` 仍依赖本机 Homebrew 动态库，不是已完成的独立分发包。Vite 提示工作台资源块超过 500kB，尚未做首屏性能验收。

## 覆盖边界

未验证真实 EMS、Linux/Windows、RTU、通用点表、多设备、规模、数据库、跨寄存器字序、实际设备控制。异常响应仅证明本次映射策略。复杂未支持 PDU 的形状会被关闭，不宣称完整 Modbus 服务器兼容性。NovaSheet 本次只验证单点显示、键盘编辑、只读与应用；Excel 多格粘贴、填充柄、边缘自动滚动、中文输入和大表性能需另行验收。
