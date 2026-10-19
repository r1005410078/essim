## Why

工作台已有点表驱动的TCP从站，但主站仅有通用同步库，无法连接外部设备周期采集或下发控制。M05先交付TCP主站，RTU另立子提案，M04遗留验收继续保留。

## What Changes

- 单目标TCP连接共享多个Unit，每Unit独立引用已应用模型。
- 四区手动／周期读取，按Unit、区域、周期合并连续完整点，不跨空洞，跳过积压轮次。
- 当前Unit待写草稿显式下发，整批预校验、逐请求结果、失败停止余项、协议确认后读回核对，绝不自动重发写入。
- 最近成功值、时间、未读取／有效／过期／失败质量；独立连接、采集、停止采集和断开操作。
- 配置超时、重连与过期倍数；显式应用断开并保留兼容读取缓存，不重新引入配置版本。
- 真实事件／报文观测、独立互操作及原生验证。

## Capabilities

### New Capabilities
- `modbus-tcp-master-runtime`: TCP主站连接、调度、读写、质量及工作台集成。

### Modified Capabilities

无。

## Impact

core新增主站领域规划及命令／事件／查询；infrastructure新增独立TCP主站所有者；Tauri仅作IPC适配，前端复用Dockview／NovaSheet。复用libmodbus四区主站API，不新增协议栈或配置版本管理，不承诺M04的千端点规模。
