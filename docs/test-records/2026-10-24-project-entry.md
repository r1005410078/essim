# 工程入口与切换实施记录（2026-10-24）

对应[提案](../../openspec/changes/require-project-startup-and-switcher/proposal.md)与[确认稿](../../openspec/changes/require-project-startup-and-switcher/assets/project-entry-design.png)。代码已接入，macOS 主要流程完成；失效路径冷启动及最终窗口拖动复核因 Mac 自动锁屏待补。不把待验任务勾选为完成。

## 实现行为

- 正式桌面启用 required-project 策略，无工程返回 PROJECT_REQUIRED，入口只挂载检查或引导页，不创建工作台 Store、Dockview 和业务订阅。纯浏览器只提供引导视觉预览，文件操作明确不可用。
- 独立无文字工程插画，左右分区打开／新建／最近列表。新建始终为空，不复制旧工程配置；文件选择器绑定主窗口，取消不触发切换。
- 工程选择器位于最左，历史与检索相邻居中，三个面板开关集中右侧。菜单、系统快捷键和命令检索复用工程流程。补齐无指针的辅助技术点击展开，方向键可进入最近工程子菜单。
- 目标先预检并持锁，必要时保护旧草稿并停止通信，确认取消保留旧工程。成功重载前端，以目标 session 重建订阅、树、索引和布局。
- 最近设置兼容旧单路径，保存 lastOpened 与最多十条路径去重列表。关闭清空 lastOpened 但保留列表；退出应用保留当前恢复入口。移出最近项不删除工程。

## IPC 门禁覆盖

所有业务 IPC 以自定义 Tauri `ProjectAccess` 参数取得工程 session 请求租约。租约覆盖异步命令，活跃请求阻止替换，旧 session 返回 PROJECT_CHANGED。切换写锁期间参数解析立即返回 PROJECT_BUSY，不阻塞原生事件循环；目录查询仍可服务旧工程。订阅仅在一次取快照期间持租约，睡眠不持有；暂时忙碌重试，工程身份变化后退出。

| 类别 | 门禁入口 |
| --- | --- |
| 配置和投影 | query_workspace、execute_workspace、preview_apply、inspect_endpoint、load_model、load_endpoint_values |
| Unit 及模型交换 | resolve_unit、preview_unit_promotion、capture_initials、export_unit_points、preview_point_table、open_point_file、export_point_file、export_point_template |
| 协议运行 | execute_slave、query_slave、export_observations、execute_master、query_master、list_serial_ports |
| 分组和批量 | preview_partition、create_partition、discard_partition、preview_batch、start_batch、query_batch、cancel_batch、discard_batch、retry_batch、query_resources |
| 订阅 | subscribe_workspace；后台每轮校验原 session |
| 工程本机文件 | read_project_local、write_project_local、load_project_document 已有 session 身份校验与同步保护 |
| 无工程允许 | query_project、prepare／commit／cancel 工程目标、remove_recent_project、close_project、窗口与退出控制 |
| 清理豁免 | unsubscribe_workspace、cancel_point_file 按独立任务令牌清理，允许旧视图释放资源 |

core 内存仓储与显式无界面执行器不依赖 Tauri 或工程磁盘，不把桌面策略强加给所有测试。

## 自动化

执行相关 core／infrastructure 全套 Rust 测试（含 tcp-runtime、vendored 与 PTY）、桌面 cargo check、cargo doc，及工作台 Vitest、tsc、Vite build。前端 48 个文件 209 项通过；新增入口不提前装配、顶部三组、拖动与交互分离、旧客户端保留原 session 的覆盖。

存储集成测试增加工程门禁、请求租约、最近迁移及十条上限、路径去重／损坏设置、新建空工程、相同路径复用。强化停机等待用例：新 IPC 参数不能等待生命周期写锁；先失败，再改 try_read，失败停机后旧目录仍可查询。标题栏先补文本选择失败用例，再阻止空白拖动默认选择行为；实际原生拖动由用户在 2026-10-25 手工确认正常。

既有配置可靠提交、引用、应用快照、草稿冲突、JSONL／SQLite 故障与 TCP／RTU 执行回归仍保留。真实运行保护、同入口不停止和重开不自动监听另有本机动态端口集成测试，已通过（动态回环端口、运行 0.24 秒量级的单次功能用例，不是性能测量）。

## macOS 已完成流程

独立测试壳 `/private/tmp/ESSim Project Entry Check.app`，标识 `dev.essim.projectentrycheck`，前端 1437，隔离构建目录 `/private/tmp/essim-entry-target`。仅使用本轮创建的 `/private/tmp/project-entry-native-A` 和 `B`。

1. 首次无最近设置显示引导，无资源树／属性／运行面板；引导页退出无需放弃临时会话确认。
2. 原生选择空目录新建 A 后进入工作台。左上工程菜单新建 B，同窗口切换到空 B。
3. 在 B 创建模型并新增一点不保存；最近子菜单选 A 时出现草稿保护。Esc 取消仍为 B、草稿未保存；再次确认成功到 A。
4. 核对磁盘：A 资源数 0；B 有一模型，正式点数 0，恢复区有 646 字节草稿。本次切换未保存正式模型、未迁移资源。
5. 关闭空 A 返回引导，最近列表 A/B 均在，lastOpened 为 null。最近列表打开 B 弹出恢复提示；恢复后点表保持未保存。
6. 工程内正常退出后冷启动，恢复 B 与其模型 Tab，并再次提示未保存草稿。
7. 原生缩至约 900×600，启动入口／最近列表／底部提示完整可达，标题栏没有重叠；三个面板开关逐一隐藏后内容占满，再可恢复。
8. Python flock 仅占用本轮 A 的 `.local/project.lock`，点击 A 显示“工程正在另一个窗口或进程中使用”，仍在引导；释放锁后未破坏工程。
9. 移出最近列表 A 后核对 A 的 project.workspace 仍存在；列表只剩 B，关闭／退出后的 lastOpened 仍为 null。

原生目录选择最初出现按钮不可用，绑定 `.set_parent(&window)` 并明确允许创建目录后重新验证通过。曾发生共享构建目录产物配置混用，改用独立构建目录后确认标题和 devUrl 正确；未把配置不符的测试壳当最终验收证据。

截图：

- [首次启动](../testing/assets/project-launcher-macos.png)
- [左侧工程菜单与三组标题栏](../testing/assets/project-switcher-macos.png)
- [原生窄窗口标题栏](../testing/assets/project-titlebar-min-window-macos.png)
- [原生窄窗口引导与最近列表](../testing/assets/project-launcher-min-window-macos.png)

## 收尾与验收边界

2026-10-25 解锁后补验：独立测试壳以不存在的 lastOpened 冷启动，显示引导及 PROJECT_IO 文件不存在错误，未挂载工作台；最近列表仍保留 B，点击后成功打开 B 并恢复测试草稿。仅修改独立测试壳设置，正式应用设置未修改。[失效路径截图](../testing/assets/project-launcher-invalid-path-macos.png)。任务 4.2 已完成。

标题栏空白双击后窗口截图从 2560×1600 变为 3356×2166，确认窗口缩放生效。引导及工作台空白拖动均未产生文本选择，但当前 CUA 仅返回窗口内部截图，不提供 macOS 窗口原点；随后为独立测试壳临时加入 WindowEvent::Moved 文件观测并重新构建，日志仅记录初始坐标 (498,352)，未取得拖动坐标变化。用户手动拖动后明确反馈“我测试了没有问题”，因此以用户原生手工确认作为拖动验收依据，任务 4.3 完成；不将探针或自动化起拖调用当作通过证据。临时探针源码已恢复，未纳入产品代码。
Windows／Linux 原生窗口行为未实测；不包含多窗口并行工程、M07 回放或应用诊断日志实现。应用日志仍为独立提案。没有本轮真实 EMS／RS-485 硬件或规模性能结论。代码尚未提交。

## 最终验证汇总

Rust 完整回归 218 项通过（含文档测试与子进程故障用例），随后补充的真实运行工程保护用例单独通过；存储集成现有 21 项。桌面 cargo check、infrastructure cargo doc、前端 209 项、tsc 和 Vite build 通过。Vite 仍报告既有大 chunk 提醒。失效恢复已补验，当前 19/19 项完成；标题栏拖动依用户手工验收，自动化仍有上述观测限制。
