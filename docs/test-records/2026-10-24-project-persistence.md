# 工程持久化实施与验收（2026-10-24）

对应[提案](../../openspec/changes/persist-project-files-and-runtime-data/proposal.md)与[使用说明](../design/project-storage.md)。在 `feat/project-persistence` worktree 实施，本记录不代表已经合并到主分支。

## 实现范围

以 `project.workspace` 打开工程，资源按稳定 ID 独立存储，显示名及虚拟目录由清单维护。保存覆盖当前内容，显式应用快照独立；不增加历史版本管理。运行事件投影到各次运行的 SQLite，原始 JSONL 保留；协议线程不执行数据库 IO。启动恢复最近工程、Dockview 布局和文档 Tab，通信默认停止。脏草稿独立暂存并显式恢复／丢弃，冲突不覆盖正式配置。工程 Markdown 支持树导航、拼音检索和只读查看，附件按原字节保存，未增加附件编辑界面。

核心通过 repository 接口工作，文件锁、文件事务、SQLite 和工程生命周期位于 infrastructure，Tauri 仅组装及适配。切换工程不持有目录锁等待协议停止，停止失败保留当前工程。独立会话令牌阻止旧工程迟到的本机状态写入。

## 自动化验证

使用仓库已安装工具，实际执行：

```sh
cargo test -p essim-core -p essim-infrastructure --features essim-infrastructure/tcp-runtime,essim-infrastructure/vendored --offline
cargo doc -p essim-core -p essim-infrastructure --no-deps --offline
cargo build -p essim-workbench --offline
# 以下在 workbench/ 执行
./node_modules/.bin/vitest run
./node_modules/.bin/tsc --noEmit
./node_modules/.bin/vite build
```

前端 47 个文件、205 项测试通过。Rust 全套覆盖 core、文件存储、TCP、RTU/PTY、录制与文档测试；共 214 项通过（包含文档测试与子进程故障用例）。类型检查、生产构建及 Rust 文档生成通过。Vite 仍报告既有的大 chunk 提醒，不作为本轮包体优化完成证据。

新增存储集成测试 16 项：配置／覆盖／应用快照往返，文档与附件保真，缺失模型、重复 ID、非法树、越界／符号链接、未来格式、外部改写、独占写、复制工程令牌、停止／打开失败保留当前工程、文档稳定 ID 与最近设置并发写。运行投影集成测试 2 项：事件筛选、Unit 关联、运行结束仍保留创建时间、流未释放时完成标记、SQLite 失败不拒绝可靠 JSONL。另有队列上限和底层事务恢复单元用例。

子进程故障矩阵真实终止在暂存、prepared、持久决策、资源安装、清单安装、删除及清理之后；另注入 ENOSPC（28）及权限错误（13）。重新打开只出现完整旧状态或完整新状态，已决前滚，未决丢弃。外部改写拒绝覆盖，提交结果不确定时阻断后续写入直到重新打开。不是物理断电或真实坏盘实验。

实际发现并先加失败用例后修复：切换工程首条命令的 projection 序号未同步，曾出现已提交后报 SEQUENCE_GAP；停止事件已经入库但上下文尚未完成；完成状态写回曾丢失创建时间。草稿保存失败保持脏标记、恢复冲突保留副本、延后恢复后继续编辑不会丢新输入均有前端用例。

## macOS 原生验收

使用独立 `ESSim Persistence Check` 测试壳，标识 `dev.essim.persistencecheck`，Vite 端口 1436；测试目录均位于 `/private/tmp`，未操作用户原有工程。通过系统文件选择器创建／打开工程，创建模型“持久化 PCS”、一个点、模拟器“持久化测试机”与 Unit 引用，保存及显式应用后退出再启动，定义与应用状态仍在。

TCP 测试地址 127.0.0.1:21532 是本机验收约定。独立 Python socket 发送 `000100000006010300000001`，收到 `0001000000050103020000`。停止并复制工程到另一目录后重新打开，同样收到 `0002000000050103020000`。SQLite 记录到 listening、connected、disconnected、stopping、stopped，原始帧保留在 JSONL。

- 新建“Modbus 验收目录”，选择 `modbus-tcp` 图标及 `#399f91`，将模拟器移动进去；清单记录归属，磁盘没有同名真实文件夹。
- 修改 Unit 名但不保存，退出后恢复草稿；界面显示未保存，正式文件仍为旧名称，运行未启动。
- 复制已停止且提交完成的工程（省略 `.local`），打开副本保留 ID、模型引用、目录与应用快照；本机布局单独重建。
- 工程内 Markdown“接入说明”通过 `jrsm` 拼音检索打开，表格及目录正常；退出冷启动恢复同一个 Tab，无重复或空白文档，未自动监听。
- 最终版本原生启停后，在应用仍打开时读取新运行 `context.json`，`ended=true`、`complete=true`；创建时间保留修复再由集成测试覆盖。
- 新增工程操作复用公共弹框：初始取消焦点、取消／Esc、原生选择器返回、退出保护、标题拖动、边角缩放、复位；窗口缩至约 900×600、弹框缩至约 320 宽后内容滚动且操作栏可见。

截图：

- [模型保存](../testing/assets/project-model-saved-macos.png)
- [副本打开与通信](../testing/assets/project-copy-open-macos.png)
- [工程 Markdown](../testing/assets/project-document-open-macos.png)
- [窄窗口](../testing/assets/project-dialog-min-window-macos.png)
- [最小弹框滚动](../testing/assets/project-dialog-minimum-macos.png)

开发期间移动 Hook 调用位置导致一次 HMR 状态不兼容，冷启动后正常；最终验证使用冷启动，不将开发热更新现象当作生产启动结果。

## 明确规模测量

macOS aarch64，本机 debug 构建，`project_persistence` 示例；10 个模型，每个 1000 点，总计 10000 点，无协议端点并发。单次短测，1000 条事件通过可靠 JSONL 后投影 SQLite，以实际 SQL 行数核对。`/usr/bin/time -l` 测量整个进程 RSS，并非隔离各步骤内存。

| 项目 | 结果 |
| --- | ---: |
| 新建工程 | 447 ms |
| 打开工程 | 218 ms |
| 保存一个模型修改 | 301 ms |
| 1000 事件 JSONL 与 SQLite 刷新 | 59 ms |
| 原始记录字节／队列峰值 | 88927 / 88927 bytes |
| 刷新后积压／SQLite 失败 | 0 / 0 |
| 整个进程时长 | 1.72 s |
| 最大 RSS | 27213824 bytes |

这不是持续吞吐、百万点、真实 EMS 或现有规模承诺的验收。后续仅上下文元数据保留逻辑有修复，不把短测数字外推到其他负载。

## 适用边界

Windows／Linux 原生流程、网络盘与云同步并发、物理断电、真实 EMS 和真实 RS-485 硬件没有在本轮验证；Unix 目录同步不能证明其他平台持久语义。测试使用本地磁盘及进程中断。M07 历史查询／回放仍未实现，不保存全量高频点值历史。工程不提供用户配置版本管理。复制／备份前需停止通信并完成提交；不宣称运行中的目录复制是一致快照。
