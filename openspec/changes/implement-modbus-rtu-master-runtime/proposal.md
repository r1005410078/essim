## Why

M05已交付TCP主站，RTU仍未接入正式运行。用户确认继续RTU主站的软件实施，多Unit共享独占串口，复用已有点表和运行语义。

## What Changes

- RTU串口打开／采集／单Unit暂停／关闭，基础8功能码与原子预校验、逐请求回验。
- 超时／CRC错误隔离、迟到数据清理、通道断开恢复，不自动重发写入。
- 串口别名与独占检查、应用／删除／退出释放、通信参数改变清空缓存。
- 真实RTU字节、CRC及诊断结果；手动07／08读取类与回送／0B／0C／11，禁止广播及改变通信状态的诊断。
- 独立PyModbus PTY软件验证，真实RS-485硬件验收保留。

## Capabilities

### New Capabilities
- `modbus-rtu-master-runtime`: 正式RTU主站运行、诊断与观测。

### Modified Capabilities

无；沿用已确认TCP主站运行规则，TCP行为继续回归。

## Impact

复用core主站命令／查询及infrastructure主站所有者，增加RTU传输适配与libmodbus安全API，不复制调度器、不改vendored源码。Tauri保持适配职责，NovaSheet样式保持。
