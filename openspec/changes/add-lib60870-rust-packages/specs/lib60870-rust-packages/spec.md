## ADDED Requirements

### Requirement: 三包和可追溯构建

系统 SHALL 在根 workspace 提供 src、sys、rs 三包，固定完整官方源码与校验，不在构建时下载依赖或探测宿主协议库。

#### Scenario: 离线 vendored 构建
- **WHEN** 使用默认 Rust 安全包构建
- **THEN** 从固定本地源码生成静态库，产物写到输出目录，上游源码不变

#### Scenario: 来源冲突与错误前缀
- **WHEN** 同时启用 vendored 与动态前缀，或动态产物不匹配版本／目标／摘要
- **THEN** 明确拒绝，不回退系统库

### Requirement: 104 安全 API 和回调边界

系统 SHALL 通过 RAII 管理 ASDU、信息对象、服务端及回调，保持线程绑定，拦截无效参数和 Rust panic。

#### Scenario: 有效 ASDU 和真实总召
- **WHEN** 使用者构建支持的 ASDU 并处理真实 TCP 总召
- **THEN** 上游库完成编码与 104 收发，安全接口不暴露悬空 C 指针

#### Scenario: 回调 panic 与停止
- **WHEN** 用户处理器 panic 或销毁正在运行的服务端
- **THEN** panic 不跨越 ABI，连接／监听在回调状态释放前关闭，后续可以重新绑定

### Requirement: 验证与范围

文档 MUST 列明安全 API 子集、源码与许可、真实验证平台，以及尚未支持的 101／TLS／完整互操作范围。

#### Scenario: 交付说明
- **WHEN** 本轮三包完成验证
- **THEN** 不把库封装测试当作工作台功能、真实 EMS 或完整标准符合性验收


### Requirement: 可直接运行的封装 Examples

系统 SHALL 提供一条 cargo run 命令执行的独立回环测试入口，消费安全 API 并用独立字节预期核对结果；失败 MUST 返回非零退出码并释放监听资源。

#### Scenario: 示例端到端通过
- **WHEN** 执行 iec104-smoke 示例
- **THEN** 核对总召确认、单点／遥测、终止、主动上送、负确认、测试帧、停止和端口重绑，输出各步骤结果
