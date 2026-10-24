## ADDED Requirements

### Requirement: 应用诊断使用独立系统目录

系统 SHALL 通过 Tauri 系统路径接口解析应用日志目录；工程切换 MUST NOT 改变日志根目录。每进程会话 SHALL 独立写入带格式标记的目录，提供打开已解析日志目录的命令；前端 MUST NOT 指定任意日志写入路径。

#### Scenario: 无工程及工程切换
- **WHEN** 应用未打开工程、打开工程或切换工程
- **THEN** 应用日志均写入同一系统日志根目录的当前进程会话，工程 runs 及 SQLite 不混入应用诊断日志

#### Scenario: 并行进程
- **WHEN** 两个应用进程同时运行和轮转
- **THEN** 两个会话的文件互不覆盖，活动目录不被另一进程清理

### Requirement: 分级及有界结构化记录

日志 SHALL 支持 TRACE、DEBUG、INFO、WARN、ERROR；发布默认 INFO、开发默认 DEBUG、第三方默认 WARN，支持 ESSIM_LOG 启动覆盖和模块过滤。每条 SHALL 为合法 UTF-8 JSON，包含时间、级别、模块、事件、消息、版本、进程、会话及序号；上下文按需使用稳定 ID。单条编码后 MUST 不超过 16 KiB。

#### Scenario: 默认及过滤
- **WHEN** 发布版以默认设置启动并产生 DEBUG 和 ERROR
- **THEN** 文件包含 ERROR 而不包含 DEBUG；以有效模块过滤重启后按过滤输出

#### Scenario: 非法设置和超长异常
- **WHEN** ESSIM_LOG 非法或异常文本超过单条上限
- **THEN** 使用默认过滤并仅一次警告；超长事件保留合法 JSON 和 truncated 标记

#### Scenario: 关键错误与诊断内容
- **WHEN** 工程恢复、配置应用或通信任务失败
- **THEN** 在拥有者边界记录适用的错误码、操作及稳定 ID，不默认序列化完整配置、点值、协议帧或文档正文；用户取消不记录为 ERROR

### Requirement: 大小轮转与有限历史

系统 SHALL 按 10 MiB 阈值轮转，每会话保留当前文件和至多九个归档，单文件超出偏差 MUST 不超过 16 KiB。非活动会话历史 SHALL 在启动及轮转清理后收敛到 100 MiB；只清理本应用可确认非活动的日志目录。

#### Scenario: 多次轮转
- **WHEN** 通过注入小阈值连续写入超过十个文件容量
- **THEN** 归档次序正确、最旧淘汰、文件数和单文件大小符合配置，剩余各行仍可解析

#### Scenario: 历史清理边界
- **WHEN** 根目录包含超预算旧会话、活动会话、无标记目录和工程运行文件
- **THEN** 仅清理可确认退出的本应用日志历史，不改动活动或无关文件；清理失败可见且不影响工程

### Requirement: 诊断写入不得阻塞协议执行

诊断 SHALL 在独立文件线程写入与轮转。整条事件队列 MUST 有界，默认最多 2048 条且载荷上限 32 MiB；满时调用方不得等待磁盘，应累计丢弃数量并在可写时输出有限摘要。

#### Scenario: 慢磁盘和队列满
- **WHEN** 文件 sink 被测试阻塞并填满队列
- **THEN** 后续诊断入队返回且 dropped 增加，协议任务继续处理，内存载荷不突破约定边界

### Requirement: Rust 异常和服务失败收集

系统 SHALL 接入 Rust panic hook，记录线程、位置及受限堆栈，保留原有 panic 行为。关键 Result 失败及任务 join 异常 SHALL 在拥有者边界记录；诊断不得改变 FFI 异常约束。

#### Scenario: 工作线程 panic
- **WHEN** 隔离测试进程中的工作线程发生可 unwind panic 且写入器仍可运行
- **THEN** 日志包含 panic 的位置和线程关联，调用者仍观察原任务失败，不被转换为成功

#### Scenario: 主线程异常边界
- **WHEN** 隔离进程触发主线程 panic 或初始化失败
- **THEN** 能运行的日志链路尽力记录并保留 stderr 兜底，测试不声称 SIGKILL、abort、native crash 或断电尾部记录保证

### Requirement: 前端异常通过统一适配器记录

前端 SHALL 在入口加载前收集 window error 和 unhandledrejection，接入 React 渲染异常及入口 import 失败；通过统一适配器向后端提交受限 DTO。相同指纹 5 秒内合并、每秒至多 20 条，后端 SHALL 再校验和限流；不得递归上报日志提交失败。浏览器预览 SHALL 明确只使用 Console。

#### Scenario: 渲染与异步异常
- **WHEN** 测试触发 Error Boundary、未处理 rejection 或入口加载失败
- **THEN** 应用记录异常类别及受限堆栈，保留原有错误界面；同一异常风暴不会产生无限 IPC

#### Scenario: 非法 DTO 和传输失败
- **WHEN** 调用方提交超长／非法级别／任意路径字段或日志 IPC 失败
- **THEN** 后端拒绝或规范化非法字段，不按外部路径写入；前端只产生有限 Console 兜底而不递归调用

### Requirement: 日志故障可见且与业务可靠性隔离

文件初始化／写入失败 SHALL 降级为 stderr，暴露 directory、fileAvailable、dropped、writeFailures 及受限 lastError，并在既有工作台诊断入口显示状态。诊断故障 MUST NOT 改变工程保存、协议运行及可靠 JSONL 门禁。

#### Scenario: 无权限及磁盘满
- **WHEN** 日志目录不可写或注入磁盘满错误
- **THEN** 状态显示不可用并累计失败，工作台和协议继续按原业务语义工作，不递归刷日志

### Requirement: 正常退出限时刷新

系统 SHALL 在关键业务停机完成后记录退出事件并刷新诊断队列，等待 MUST 不超过 2 秒。刷新失败不得永久阻塞退出，不承诺诊断日志具备审计或掉电持久性。

#### Scenario: 正常退出和卡住的 sink
- **WHEN** 用户正常退出，分别使用可写 sink 和卡住 sink
- **THEN** 可写场景能读取退出记录；卡住场景在期限后降级 stderr 并继续退出，不修改工程可靠提交结果
