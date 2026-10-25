## Context

当前桌面入口在 Tauri setup 中组装 WorkspaceEngine，应用没有统一日志初始化。仅少量 `eprintln!`；React Error Boundary 只显示错误文字，没有 componentDidCatch 记录。工程 `runs/`、SQLite 和可靠 JSONL 用于协议执行，不能替代进程诊断。core 不应依赖 Tauri 或文件系统，协议所有者线程不应等待诊断磁盘 IO。

## Goals / Non-Goals

**Goals:** 应用独立日志目录、可调整分级、有界轮转、关键错误及前后端异常收集、可关联的上下文、正常退出刷新和写入降级提示。

**Non-Goals:** 日志浏览器、云端上传、自动遥测、原生 minidump、捕获 SIGKILL／断电／C 库段错误的完整堆栈、工程格式改动、全量点值与协议帧复制、重做协议录制。不开设置页面，不变更既有 Dockview 布局。

## Decisions

### 1. 系统目录及多进程隔离

桌面组装层调用 Tauri `app_log_dir()`，传入 infrastructure 的日志初始化接口；路径禁止由前端任意指定。当前应用标识 `dev.essim.workbench`，预期路径为：

| 平台 | Tauri 默认应用日志目录 |
| --- | --- |
| macOS | `~/Library/Logs/dev.essim.workbench/` |
| Windows | `%LOCALAPPDATA%/dev.essim.workbench/logs/` |
| Linux | `$XDG_DATA_HOME/dev.essim.workbench/logs/`，未设置时 `~/.local/share/dev.essim.workbench/logs/` |

实际以运行时解析值为准，不手工拼用户名、产品名称或跨平台目录。切换／关闭工程不切换应用日志位置。

每进程会话使用 `sessions/<UTC时间>-<PID>-<随机后缀>/application.jsonl`，持有会话锁；轮转后为 `.1`…`.9`。并行测试壳或多个进程不争抢同一文件。命令检索增加“打开应用日志目录”，后端只打开已解析目录，不接受调用者路径，不读取或上传日志。

替代方案：工程内 logs 不符合应用故障独立诊断；单一共享文件易发生多进程轮转冲突。首版使用会话目录，不建立跨进程日志服务。

### 2. 分级和记录格式

JSON Lines，UTF-8，单条独立 JSON，字符串中的换行转义。固定字段：UTC 时间（毫秒）、level、target／模块、event、message、appVersion、sessionId、PID、线程名／ID、进程内 sequence；可选 operationId、projectId、endpointId、unitId、errorCode。sequence 用于观察缺口，不建立审计或事件溯源承诺。

TRACE／DEBUG／INFO／WARN／ERROR 五级；发布默认 INFO、开发默认 DEBUG，第三方库默认 WARN。`ESSIM_LOG` 启动变量支持全局级别及模块过滤（例如 `info,essim_infrastructure=debug`）；非法值回退默认并只警告一次。固定应用标识之外不依赖项目名称。改变变量后重启生效，本轮不加配置 UI。

INFO：启动、正常退出、工程打开／切换、保存／应用的结果摘要、端点启停。WARN：可恢复异常、恢复冲突、自动重连状态变化。ERROR：启动失败、存储／应用／运行任务失败及未捕获异常。DEBUG／TRACE：显式启用的诊断细节，不逐请求打印协议帧。预期取消不报 ERROR，普通输入校验按需 DEBUG；同一错误只在所有权边界记录一次，避免后端与前端反复报同一个失败。

禁止默认序列化整个命令／配置／工程、点值、原始帧、文档正文或任意 JS 对象；优先稳定 ID 与错误码。堆栈限制长度并归一化本机用户根目录，URL 去 query／fragment；错误自由文本采用统一裁剪和已知敏感字段清理，但不宣称任意第三方异常文本都能完全脱敏。前后端消息必须经同一后端约束后写入。

### 3. 组件选型与非阻塞写入

优先 `tracing` + `tracing-subscriber` 作为 Rust 统一接口，单向 `tracing-log` 兼容依赖的 log 记录，禁止反向桥接形成环路。评估 `tracing-appender` 的非阻塞 writer 配合 `rolling-file` 大小轮转；实施时锁定实际可用版本并核实许可与目标构建。Writer/过滤生命周期由桌面组装层持有，基础设施不自行安装第二个全局 subscriber；无界面执行器保持自行选择 subscriber 的能力。

Tauri 官方 log 插件支持系统目录和大小轮转，可作选型比较，但本轮重点是协议线程非阻塞及明确队列上限，不能只接默认同步 sink 就认为满足目标。不同时安装两套全局 logger。若候选 writer 没有写入错误可观测接口，增加小型错误观察包装，不改写成熟轮转算法。

每条最终编码上限 16 KiB，包含堆栈在内；裁剪仍保持合法 JSON 并加 truncated。后台队列最多 2048 条，以整条编码后的记录为边界，队列载荷上限 32 MiB；不使用无界 channel。满时拒收并累计 dropped 数，不等待文件线程；工作线程持有文件并处理轮转。下一次可写时生成有界丢弃摘要，诊断状态保留累计计数，不为每次丢弃再生成一条日志。编码后的单事件超过上限先裁剪，不把非阻塞库内部按 write chunk 的数量误当成整条记录数量。

### 4. 大小轮转和保留

单文件目标上限 10 MiB，写下一条前判断并轮转；允许的边界偏差最多一条 16 KiB（若候选库可严格控制则不超出）。每会话当前文件加 9 个归档，最旧归档淘汰，不压缩。启动及轮转时清理已退出会话：按时间从旧到新删除，直到非活动会话总量不超过 100 MiB；只能删除带本应用格式标记且可获得会话锁的目录。活动会话不清理，因此上限为活动会话数 × 约 100 MiB + 非活动历史 100 MiB。清理失败报告状态，不删除其他应用文件或工程记录。

不设置时间轮转或七天保留，避免时间跳变与大量空文件；按字节和数量限制足以覆盖当前需求。默认值集中定义，测试可注入小阈值，无需生成百兆测试文件。

### 5. 异常与操作边界

- Rust panic hook 在启动早期安装，记录线程、位置、受限 message 和可获取的 backtrace，调用原 hook；不吞 panic、不改变 unwinding 或 FFI 安全契约，不在 hook 内使用会再次 panic 的锁／unwrap。主线程／工作线程测试用子进程隔离。
- 显式 join／spawn_blocking／异步任务返回异常由拥有者记录，包含 operationId；线程 panic 的 hook 与 join 结果合并或标识关联，避免相同堆栈刷屏。关键初始化失败及退出刷新失败补日志；不能只覆盖 panic 而忽略 Result 错误。
- 前端在入口动态 import 前安装 window error／unhandledrejection；React componentDidCatch 记录 error 和 componentStack；入口 import catch 也记录。使用共享诊断适配器，Tauri invoke 仅在适配层出现；纯浏览器预览只用本地 Console，明确不声称写系统日志。
- 前端不覆盖全部 console，既有显示错误保持原样；已知业务拒绝不当成未捕获异常。相同指纹 5 秒内合并，前端每秒最多 20 条，单次 IPC 载荷最多 16 KiB；后端再次校验字段、级别、长度及限流。日志提交失败只写一次 Console／计数，不递归调用日志提交。
- 记录 engine 组装前先完成日志路径与文件初始化；路径解析前的早期错误使用受限 stderr 缓冲，初始化后补写。若 Tauri 连 setup 都无法到达，仍只能保证 stderr 兜底，不承诺原生加载器故障落盘。

### 6. 失败、退出和可见性

初始化目录／写文件失败时应用继续启动，只保留 stderr；诊断状态通过后端查询暴露 fileAvailable、directory、dropped、writeFailures 和受限 lastError，在现有问题／资源入口呈现“应用日志不可用／有丢弃”。不得把诊断失败变成协议故障、配置保存失败或可靠 JSONL 失败。写入线程报错经独立状态通道更新，避免日志系统递归记录自身失败；重复错误限频。运行中恢复写入以低频重试，不能忙循环。

正常退出在结束关键业务任务后先记录退出事件，再刷新诊断队列，最多等待 2 秒；刷新未确认反馈 stderr 和诊断状态，继续退出，不让日志故障永久阻止关闭。panic 仅尽力入队并交给可运行的 writer，不承诺异常退出最后一条、SIGKILL、abort、断电或 native crash 一定落盘。测试核对异常记录和边界，不把队列 flush 等同 fsync／掉电安全。日志是可丢的诊断资料，工程协议录制仍按原有可靠失败门禁处理。

## Risks / Trade-offs

- [队列满时 ERROR 也可能丢失] → 明确有界非阻塞优先级，显示累计丢弃，异常路径尽力记录；不冒充可靠审计。
- [多进程使总量高于单会话上限] → 活动锁隔离，明确总量公式，非活动历史自动收敛。
- [磁盘故障／异常退出尾部丢失] → stderr 降级、状态可见、正常退出限时刷新；不声称完整崩溃转储。
- [第三方日志包含本机信息] → 默认 WARN、限制模块与字段、裁剪清理；不自动上传，用户自行检查分享内容。
- [前端异常风暴或提交递归] → 双端限流、去重、IPC 校验、独立失败通道。
- [非阻塞库仅有条数或写 chunk 限制] → 在整条记录层封装并验证最大载荷，不能仅照搬默认配置。

## Migration Plan

新增日志设施并逐层接入入口、关键服务、前端异常；不迁移旧 stderr、工程或 SQLite。用户原来的运行目录完全保留。macOS 原生先验收，同时编写路径／轮转／故障自动测试；Windows／Linux 未实测部分明确留待验证。关闭新日志设施可回退 stderr，不回滚工程数据。

## Open Questions

无产品范围阻塞项。具体依赖版本及 writer 的错误反馈适配在实施首项核实；若库无法满足非阻塞／保留边界，先更新本设计再替换组件，不降低验收条件。

## References

- [Tauri 官方日志文档](https://v2.tauri.app/plugin/logging/)：系统目录与插件轮转能力。
- [tracing-appender WorkerGuard](https://docs.rs/tracing-appender/latest/tracing_appender/non_blocking/struct.WorkerGuard.html)：异步队列及异常退出尾部丢失边界。
- [rolling-file 官方 API](https://docs.rs/rolling-file/latest/rolling_file/)：按大小轮转、有限归档及与 non_blocking 组合。
- [tracing-log 官方 API](https://docs.rs/tracing-log/latest/tracing_log/struct.LogTracer.html)：log 到 tracing 的单向桥接。

## 实施选型补充（2026-10-25）

锁定 tracing 0.1.44（MIT）、tracing-subscriber 0.3.23（MIT）、tracing-log 0.2.0（MIT）、rolling-file 0.2.0（MIT/Apache-2.0）。源码核对发现 rolling-file 自动轮转失败会 stderr 后继续写，因此关闭其自动条件，在每条写入前显式调用 rollover 并处理 Result；不复制其归档算法。tracing-appender 的 chunk 队列不能直接表达整条载荷上限，采用 std 有界完整记录队列及独立 writer，控制消息单独限时通道；文件锁使用 Rust 标准库 File::try_lock。
