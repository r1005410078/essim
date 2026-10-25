# 应用诊断日志实施与验收（2026-10-25）

实施分支：`feat/application-diagnostic-logging`，独立 worktree：`.worktrees/application-diagnostic-logging`。按[提案](../../openspec/changes/add-application-diagnostic-logging/proposal.md)实施，不改工程格式、SQLite 或协议可靠 JSONL 的门禁。

## 自动化证据

- 锁定 tracing 0.1.44、tracing-subscriber 0.3.23、tracing-log 0.2.0、rolling-file 0.2.0；源码核对许可与轮转错误接口，macOS 构建通过。rolling-file 自动轮转会忽略错误继续写，故使用显式 rollover 并处理错误；归档移动仍由库完成。完整记录队列自行使用 std 有界通道，不叠加 tracing-appender 的 chunk 队列。
- 日志专项覆盖过滤回退／模块覆盖、UTF-8 与转义后 16 KiB 上限、未知字段和超长 DTO、完整记录队列满立即返回、归档顺序及逐行 JSON、活动锁及未知目录保护、权限失败与恢复、磁盘满、轮转失败、慢 sink 和刷新超时。
- 子进程验证主线程／工作线程 panic：保留失败或 join Err，原 hook 仍执行，落盘 panic 位置与线程；早期错误补写，tracing 与 log 桥接无重复。两个子进程同时轮转，活动会话不被清理，退出后历史可收敛到注入预算。
- 后端 IPC 测试覆盖非法级别、任意路径字段、超长文本及每秒二十条限制。前端覆盖五秒去重、每秒二十条、转义后的请求上限、传输失败不递归、任意对象不序列化及问题状态呈现。
- Rust core／infrastructure 全量回归 235 项通过（包含 doctest），覆盖工程存储、TCP／RTU、可靠录制、停机和应用门禁。桌面测试 11 项通过；前端 214 项通过；类型检查、生产构建、cargo doc 及 OpenSpec strict 校验通过。最终 Rust 合计 246 项（core／infrastructure 235、桌面 11）；Vite 仍提示已有大 chunk 警告。

以上故障通过专用 sink、注入错误及隔离子进程制造，没有修改真实系统磁盘权限或填满用户磁盘。

## macOS 原生

环境：macOS 26.5.2，Darwin arm64，Rust 1.99.1，调试构建。独立壳 `ESSim Logging Check`，标识 `dev.essim.loggingcheck`，本地前端端口 1439；只创建 `/private/tmp/essim-logging-project-A` 验收工程。

1. 无工程引导页即出现 `application_start`，目录实际为 `~/Library/Logs/dev.essim.loggingcheck/sessions/<会话>/application.jsonl`。
2. 新建工程、创建模型与端点、保存实例点表、显式应用、真实从站监听与停止均正常；文件记录操作摘要、operationId、endpointId，未复制点值或命令正文。空有效点表启动仍按原门禁拒绝，关联错误码及 Unit；最终实现将这类校验拒绝归为 DEBUG。
3. 关闭工程回到引导，再通过最近工程打开，`project_opened` 仍写入同一进程会话。顶部 `>打开应用日志目录` 实际在 Finder 打开固定系统目录。
4. 独立异常入口触发 window error、unhandled rejection、失败 import 和 React 渲染异常；四类事件都经 IPC 落盘。Boundary 仍显示“工作台加载失败”；状态查询为 fileAvailable=true、dropped=0、writeFailures=0。
5. 普通工作台退出经过草稿保护后，尾部存在 `application_exit`；异常测试壳关闭后也有退出记录。

[正常运行截图](../testing/assets/application-logging-macos.jpg) · [异常与状态截图](../testing/assets/application-logging-exceptions-macos.jpg)。原生壳使用正式日志接口；异常仅在专用测试入口和子进程触发，正式业务入口无故障按钮。

## 规模与测量边界

同一 macOS arm64 环境，debug 构建、默认 DEBUG 过滤，提交 INFO 事件，在专用临时目录运行 `application_log_benchmark`：单线程连续入队 50,000 条，每条 message 为 1000 ASCII 字节；JSON 元数据另计；最终代码总入队时长 4695 ms，P50 91,334 ns，P95 99,167 ns，P99 137,000 ns，累计 dropped=0、writeFailures=0，退出刷新确认。这是一次微基准样本，不是 release 性能，不测真实 EMS、设备连接数或协议吞吐，不宣称零影响。

专项慢 sink 测试用两条队列，文件写入被阻塞后继续提交 1000 条；至少 998 条被丢弃且调用在一秒断言内返回；刷新注入 30 ms 截止，250 ms 断言内返回失败，随后解除阻塞并回收线程。归档测试使用 120 字节阈值写入 500 条 JSON，验证十文件上限、连续剩余序号及合法 JSON；并行子进程用 2048 字节阈值，各提交 500 条。这些是受控故障／边界验证，不等于真实磁盘延迟分布。

## 限制

Windows／Linux 原生路径、文件锁、目录打开和退出尚未实测。使用标准库文件锁及跨平台代码，但不以 macOS 通过替代这些验收。文件系统本身失效时不保证最后一行完整；SIGKILL、abort、C 段错误和断电不保证尾部。诊断可丢且不是审计；协议可靠记录沿用既有失败策略。

## 可复核命令

```sh
cargo test -p essim-core -p essim-infrastructure -p essim-workbench --features essim-infrastructure/tcp-runtime,essim-infrastructure/vendored --offline
cargo doc -p essim-infrastructure -p essim-workbench --features essim-infrastructure/tcp-runtime,essim-infrastructure/vendored --no-deps --offline
# 在 workbench 目录，复用已安装依赖；本轮 worktree 未重新安装共享 node_modules。
./node_modules/.bin/tsc --noEmit
./node_modules/.bin/vitest run
./node_modules/.bin/vite build
# 在仓库根目录
OPENSPEC_TELEMETRY=0 openspec validate add-application-diagnostic-logging --strict
```

最终注册窗口授权补充后重建原生壳，error 提交和状态查询再次成功、无丢弃；关闭后尾部再次出现 application_exit。没有运行 Windows／Linux 原生验收，也没有提交或合并本分支。
