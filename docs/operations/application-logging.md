# 应用诊断日志

应用日志用于排查客户端、工程操作和后台任务异常；工程中的 `runs/`、SQLite 和可靠 JSONL 继续记录协议执行，二者互不替代。

## 位置与打开

在顶部命令检索中输入 `> 打开应用日志目录`。目录由 Tauri 的 `app_log_dir()` 解析，调用方不能提供路径。正式 macOS 应用通常位于 `~/Library/Logs/dev.essim.workbench/`；测试壳使用自己的应用标识。Windows／Linux 路径由同一系统 API 解析，尚未完成原生验收。

```text
系统应用日志目录/
└── sessions/
    └── <UTC时间>-<PID>-<随机后缀>/
        ├── format
        ├── session.lock
        ├── application.jsonl
        ├── application.jsonl.1
        └── … application.jsonl.9
```

每个进程创建独立会话，同一进程所有工程窗口共享；打开、关闭工程不切换日志会话。每条为 UTF-8 JSON，包含 UTC 毫秒时间、级别、模块、事件、消息、应用版本、会话、PID、线程和分配序号。操作按需关联稳定身份及错误码；序号用于发现缺口，不承诺跨线程文件排列等同序号分配顺序。

## 分级

发布构建默认应用 INFO、开发构建 DEBUG；第三方默认 WARN。启动前可设置 `ESSIM_LOG`，例如：

```sh
ESSIM_LOG='info,essim_infrastructure=debug' pnpm --filter @essim/workbench dev:tauri
```

支持 `trace/debug/info/warn/error`、全局级别以及 `模块=级别`，最长模块前缀优先；非法配置整体回退默认值，并记录一次警告。更改后重启生效。日志不是点值、报文、文档或工程内容的副本；默认操作摘要只记录结果和稳定 ID。

## 容量与故障

单条最终编码不超过 16 KiB；裁剪后仍为合法 JSON，并标注 `truncated`。完整记录队列至多 2048 条（32 MiB 载荷，另有一条正在处理）；满时立即拒收，ERROR 也可能丢失。丢弃摘要正常写入时至多每秒一条，退出时尽力补最后累计值。

单文件目标 10 MiB，写入下一条前显式轮转，保留当前文件加九份归档。启动及轮转清理带正确格式标记、可获得锁的非活动会话，历史预算 100 MiB。未知目录、符号链接、活动会话及工程文件不清理；活动会话不计入历史预算，因此多进程总量可超过 100 MiB。

初始化、写入或轮转故障通过独立状态报告，限频输出 stderr，文件线程每 30 秒尝试恢复。工作台“问题”面板显示不可用、累计丢弃及写入失败。诊断故障不改变保存、应用或可靠协议录制的业务结果。

正常退出在业务保护完成后记录退出事件，最多等待两秒排空／刷新，超时继续退出。flush 不等于 fsync，也不保证 SIGKILL、abort、native crash、断电时的尾部记录。日志是可丢诊断资料，不是审计记录。

## 异常与分享

Rust 保留原 panic hook 和 unwinding 行为，并尽力记录位置、线程及受限堆栈；不会放宽 FFI 边界。前端在业务入口动态加载前安装全局 error／unhandledrejection，React Boundary 和入口 import 失败共用受限适配器。相同异常五秒去重，前端每秒最多二十条，后端进程级再次限流。传输失败仅一次 Console 兜底；浏览器预览只有 Console，不写系统目录。

消息会裁剪、替换本机 HOME、去除 URL query／fragment，并清理约定的 `password=`、`token=`、`secret=`、`authorization=` 片段；不保证任意第三方自然语言完全脱敏。应用不自动上传日志，分享前请自行检查内容。

## 开发与复核

Rust 接口位于 `essim_infrastructure::diagnostics`，宿主显式持有 WriterGuard 并安装唯一 subscriber；core 不初始化日志，无界面宿主可自行选择 subscriber。

`workbench/diagnostic-regression.html` 是独立测试壳入口，不被生产入口引用。将独立 Tauri 测试配置的 devUrl 指向它，可手动触发 window error、Promise rejection、入口 import 失败和 React Boundary；只对独立测试壳使用。微基准见 `application_log_benchmark` example，必须传专用测试目录，其结果不能当作协议吞吐报告。

本轮证据见[macOS 实施记录](../test-records/2026-10-25-application-logging.md)。
