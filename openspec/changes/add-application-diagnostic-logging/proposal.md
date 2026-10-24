## Why

应用已具备工程和协议运行记录，但 Rust 报错仅零散输出到 stderr，前端缺少统一异常收集。打包后用户难以定位启动、工程恢复、通信任务与界面异常，需要独立于工程的、有界的应用诊断日志。

## What Changes

- 日志写入 Tauri 解析的系统应用日志目录，与工程 `runs/`、SQLite 和协议原始录制分开；提供“打开日志目录”命令。
- 统一 Rust、前端异常与关键操作的结构化日志，支持 TRACE／DEBUG／INFO／WARN／ERROR；发布默认 INFO、开发默认 DEBUG，可通过启动环境变量调整。
- 支持按大小轮转和有限保留；后台有界写入，记录队列丢弃和磁盘故障，不阻塞协议线程。
- 捕获 Rust panic、工作线程／异步任务异常、前端全局 error／unhandledrejection、React 渲染异常及入口加载失败；正常退出限时刷新。
- 记录时间、进程／会话、模块、事件和错误码及适用的工程／端点稳定 ID；限制消息大小，不默认复制工程内容、点值或协议帧。
- 日志不可用时降级 stderr 并在工作台显示诊断状态，不改变协议可靠录制的失败策略。

## Capabilities

### New Capabilities

- `application-diagnostic-logging`: 系统目录、结构化分级、轮转保留、异常收集、前端适配与故障可见性。

### Modified Capabilities

无。既有工程配置存储及可靠运行录制语义保持不变。

## Impact

- `workbench/src-tauri`：日志生命周期、系统路径解析、诊断命令及命令边界接入。
- `crates/infrastructure`：不依赖 Tauri 的诊断写入基础设施，替换相关 stderr 并接入任务异常；core 不增加文件写入或日志初始化依赖。
- `workbench/src/shared`、入口与 Error Boundary：前端统一诊断接口、异常监听和去重限流。
- 评估并锁定 tracing／tracing-subscriber／tracing-appender、log 桥接和成熟大小轮转组件；Tauri API 仍局限组装与通信适配层。
- 更新使用说明与验收记录，不新增日志浏览器、云端上报、崩溃转储或历史回放功能。
