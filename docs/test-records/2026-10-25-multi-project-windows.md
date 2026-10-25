# 多工程窗口实施与验收记录（2026-10-25）

工作分支 `feat/independent-project-windows`，目录 `.worktrees/independent-project-windows`。从 main 7488092 建立，复制当时未提交的启动页尺寸／插画修正及已确认提案作为基线；主目录未改动。本轮约定范围已完成 macOS 验收，未验证平台及证据边界见下文。

## IPC 与资源归属矩阵

| 入口或状态 | 归属与校验 |
| --- | --- |
| query／execute_workspace、preview_apply、inspect、model、Unit 及值查询 | 实际 WebView label → ProjectContext Engine；ProjectAccess 校验该引擎 session／请求租约 |
| slave／master／串口、partition、batch 命令 | 同一窗口解析；BatchCoordinator 和运行记录随工程独立 |
| 点表预览／导入／导出／模板／单元导出 | 同一窗口 Engine 与 PointFileJobs，原生选择器以调用窗口为父窗口 |
| subscribe_workspace／unsubscribe_workspace | 窗口独立 Subscriptions，后台持原工程 session；取消 ID 只能访问本窗口集合 |
| cancel_point_file | 窗口独立任务表，不能取消另一窗口同名任务 |
| query_project、prepare／commit／cancel、close | 窗口独立 ProjectSession 与 Engine；进程级规范路径注册／预约用于去重 |
| read／write_project_local、load_project_document | 按窗口解析引擎，再由原有 session 校验和工程路径解析保护 |
| 退出保护、窗口边界 | 每窗口独立许可、固定引导和工作台边界快照 |
| 菜单保存／编辑／工程操作 | 最近活动的存活窗口；不再硬编码 main |
| 最近设置、全应用退出 | 仅这些由进程统一协调，退出确认后才开始停机 |

## 当前自动化证据

独立路径预约测试覆盖同路径竞争、打开失败释放／重试、closing 拒绝与不同目录并行。上下文测试创建两个真实磁盘工程，验证跨工程 session 拒绝、A 活跃租约阻止 A 关闭但不阻止 B 关闭、退出准备保留工程。首次桌面测试通过 8 项，前端入口与窗口适配通过 7 项；完整回归及原生多窗口流程仍进行中。

旧版订阅 Drop 测试存在误混入的无定义 engine 引用，本轮恢复为实际订阅线程回收用例。主目录 pnpm workspace 检查与 worktree 链接不匹配，未清理或重装主目录依赖；使用现有 node_modules 中真实的 tsc／Vitest／Vite 二进制执行 worktree 检查。

## 完整检查与真实通信

- core／infrastructure 完整回归通过 220 项（38 个测试目标汇总，含文档测试／故障子进程输出），执行 `cargo test -p essim-core -p essim-infrastructure --features essim-infrastructure/tcp-runtime,essim-infrastructure/vendored --offline`。端口及 PTY 用例在获得本机能力的运行环境执行。
- 桌面侧 10 项通过：路径预约、符号链接去重、同目录竞争／失败释放、不同目录独立、session 隔离、A 请求租约不阻塞 B 关闭、部分退出准备失败仍保留全部工程、重复确认及过期超时身份、订阅停止和回收。
- 前端 48 文件 211 项通过，tsc 和 Vite 生产构建通过；仍有既有大 chunk 提醒。新增事件监听目标测试，禁止业务命令默认 Any 监听。
- 新增真实 Modbus 双工程用例：macOS 本机回环、两个随机空闲 TCP 端口、各一端点／Unit／holding 点，相同业务 ID、不同初值 3／7。B 先使用 A 端口，返回占用错误且 A 仍返回 3；显式为 B 改端口后 A/B 分别返回 3／7。A 退出准备及关闭后 B 仍返回 7。单次用例约 0.77 秒，为功能与隔离验证，不是规模或独立 EMS 兼容性结论。

## macOS 原生流程

独立测试壳 `/private/tmp/ESSim Multiwindow Check.app`，bundle ID `dev.essim.multiwindowcheck`，前端 1438。仅操作本轮 `/private/tmp/essim-multiwindow-A`、`B`，不修改正式应用设置。测试过程中可按需重启我创建的独立壳，正式工程和自动暂存的测试草稿保留。

1. 从固定引导页用原生目录选择器创建 A；A 的“新建工程”在第二窗口创建 B，A 不被替换。
2. B 创建独立模型并新增未保存点，从最近列表打开 A 直接激活原 A。A 不含 B 模型。
3. A 创建不同模型、加点并 Cmd+S；磁盘 A 模型正式点数 1，B 仍为 0，B 的一点保持未保存草稿。
4. A 显式选择“在当前窗口打开工程”，通过系统文件选择器打开已由 B 承载的入口，直接激活 B，不替换 A、不出现切换保护，B 草稿仍在。
5. 全应用退出逐窗确认，取消后保留窗口和草稿；正常退出在 B 确认后继续要求 A 确认，全部确认才退出。退出后 A/B 文件锁均释放，lastOpened 保持 B。
6. 冷启动仅恢复 B 并提示恢复未保存草稿，未恢复 A。模型标签与布局恢复正常。
7. 单独关闭 A 仅释放 A 锁，B 锁仍在。修复后自动激活 B，B 显示未保存模型且不出现退出弹框。

[原生 B 工作台及未保存草稿](../testing/assets/multi-project-B-macos.png)。多窗口正常退出不通过逐窗关闭路径写空 lastOpened。

## 验收中发现并修复

- 已保护窗口调用 close 会再次被前端关闭拦截，停在忙碌弹框；改为后端保护完成后 destroy，未绕过工程停机和刷新。
- Tauri 事件默认 Any 接收方导致其他窗口收到本不属于它的命令／关闭确认；保存、编辑、工程及退出确认均显式绑定当前 Window target，错误可定向发送，全应用取消和最近变更才广播。
- macOS 销毁动态窗口后不会自动选下一个 key window。原生采样主线程处于事件等待，临时窗口事件探针确认 B 仍 visible、非 minimized；不是协议锁死。关闭完成后显式激活另一个存活窗口，实际复测可直接回 B 且草稿未丢失。临时探针已从源码移除，未作为应用诊断日志交付。
- 退出准备只停机／刷新但保留工程及文件锁，避免后续工程退出失败时先前窗口已失去工程。无响应确认超时仅取消退出，不强制退出；旧事务超时按事务序号隔离。

## 剩余及边界

当前 22/22 项完成。用户于 2026-10-25 手工确认：A 最小化后，从 B 的最近工程选择 A，能恢复并激活原 A 窗口，且没有重复窗口。该项采用用户手工验收证据；自动化返回的 AX 树不能可靠区分最小化状态，不将自动化调用成功当作通过依据。Windows／Linux 原生行为未实测；不包含跨窗口拖放、恢复所有窗口、多进程隔离、应用日志或新增协议能力。本记录保留验收时的独立分支边界，提交与合并状态以 Git 记录为准。
