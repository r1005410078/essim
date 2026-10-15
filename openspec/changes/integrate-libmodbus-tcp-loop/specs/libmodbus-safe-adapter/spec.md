## ADDED Requirements

### Requirement: 固定依赖与可复现构建
适配器 SHALL 以 libmodbus 官方 v3.2.0 为候选构建基线，记录提交、来源校验、头文件/运行库版本及许可；候选存在影响闭环的缺陷时 MUST 记录回归证据并更新设计后选择修复版本。构建与运行 SHALL 明确诊断缺少依赖、版本不匹配及不支持平台，不得静默回退到假数据。库接入 SHALL 位于基础设施，核心 MUST NOT 依赖 C 库或 Tauri。

#### Scenario: 缺少 C 库或构建工具
- **WHEN** 构建环境没有候选库、头文件或必要构建工具
- **THEN** 提供依赖定位或构建步骤，验证报告记录未完成；不把仅 Rust 编译通过写成协议接入成功

#### Scenario: 平台未实际验证
- **WHEN** 只在 macOS 执行构建与闭环
- **THEN** 报告只声明 macOS 已验证，Linux/Windows 项保持待验证

### Requirement: C 资源安全所有权
Rust 封装 MUST 集中限制 unsafe，验证空指针、缓冲区与映射边界，以 RAII 唯一释放上下文、mapping 和套接字。协议 worker SHALL 独占 C 对象，前端与其他线程不得直接访问指针或修改映射。所有权交接 MUST 避免 double close，不得未经证明提供 Send/Sync；任何引入的 C 回调 MUST 阻止 Rust panic 跨 ABI。

#### Scenario: 初始化中途失败
- **WHEN** 上下文已创建但 mapping 或监听创建失败
- **THEN** 所有已获得资源按所有权规则释放一次，返回可定位错误

#### Scenario: 运行值跨线程更新
- **WHEN** 工作台请求修改 SOC，协议 worker 同时处理读取
- **THEN** 更新通过消息序列化，C mapping 只由 worker 修改，更新确认后的读取返回新值

### Requirement: 有界等待与停机顺序
适配器 SHALL 提供有界 accept/receive 等待与受控 I/O 唤醒，正常停止 MUST 在 1s 内完成。停机 SHALL 依次阻止新操作、唤醒 I/O、由 worker 清理连接及 C 资源、释放监听并等待线程退出；只有完成后报告 Stopped。C 阻塞调用不得占用前端 IPC 主处理线程。

#### Scenario: 无客户端时停止
- **WHEN** 实例正等待 accept，用户停止实例
- **THEN** 1s 内返回停止完成，原端口可被新实例重新监听

#### Scenario: 空闲或半帧客户端时停止
- **WHEN** 客户端已连接但未发请求，或只发送半个请求后停住，用户停止实例
- **THEN** 1s 内完成资源释放，不等待客户端主动断开

#### Scenario: 客户端不读取响应
- **WHEN** 响应发送路径受阻且用户请求停止
- **THEN** 受控唤醒使 worker 在 1s 内退出，失败发送不记录为已成功响应

### Requirement: TCP 流与非法报文处理
适配器 SHALL 使用 libmodbus 处理请求接收与响应，不另建产品协议栈；同时 MUST 验证固定版本在分段、合并与非法长度情况下的实际行为。非法报文 SHALL 在有限超时内关闭当前连接，不越界访问、不改变 SOC、不破坏监听。已知缺陷导致用例失败时 MUST 阻止该候选版本被标为闭环完成。

#### Scenario: 分段与合并请求
- **WHEN** 一个合法请求拆成多次发送，或两个合法请求合并发送
- **THEN** 分段请求完整后返回一次正确响应；合并请求返回两次独立且事务号匹配的正确响应

#### Scenario: 非法 MBAP 或截断 PDU
- **WHEN** 请求声明非法长度、非零协议标识符、长度与功能不一致，或报文未完成即断开
- **THEN** 无内存越界且 SOC 不变，当前连接在有界时间内关闭，后续新连接正常读取

### Requirement: 独立互操作证据
交付 SHALL 至少包含一个非 libmodbus 实现客户端的互操作记录，注明工具与库版本、环境、端点、点映射、原始报文、预期与实际结果。自写客户端、库自己的客户端及单元测试 MUST NOT 作为完整独立兼容性证明；未测试的 EMS、平台和规模 MUST 标明边界。

#### Scenario: 独立工具闭环记录
- **WHEN** 用固定版本独立客户端读取 650，工作台修改为 700 后读回并测试越界和重连
- **THEN** 保存可复现步骤及对应报文、结果；未使用真实 EMS 时不声明 EMS 互操作通过
