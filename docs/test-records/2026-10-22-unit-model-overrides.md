# 模型到 Unit 单层覆盖实施与验证

日期：2026-10-22。关联 [OpenSpec 提案](../../openspec/changes/add-unit-model-overrides/proposal.md)、[任务](../../openspec/changes/add-unit-model-overrides/tasks.md)、[使用说明](../unit-model-overrides.md)。本记录只覆盖本次增量，不结束 M02／M03 原有遗留项、M04／M05 真实串口验收或 M06 容量目标。

## 实施结果

已接入创建时搜索／新建模型并原子绑定首个 Unit；单层字段补丁、局部新增／隐藏点；按有效定义校验的配置保存；恢复、选择性回写及另存独立模型。共享回写采用短生命周期内容基线复核，不建立配置版本。各 Unit 有独立配置草稿，保存／回写不改变已应用运行定义；实际映射和初值在完整端点显式应用后生效。

运行值捕获要求已应用映射一致及确认质量；固定预览确认后只生成初值草稿，不发送协议请求。主从运行和复位读取独立有效快照，缓存按完整内容共享、运行值独立。新查询及命令通过统一前端适配器与 Tauri 命令接入，核心不依赖窗口。有效表展平导出沿用模型格式。

已有工作区包含 M04／M05／M06、导航及属性栏等未提交修改，本轮未回退这些修改，没有提交或归档。全量测试数字包含工作区既有功能；不是本轮新增测试数量。

## 自动化

实际使用命令：

```sh
cargo test -p essim-core --offline
cargo test -p essim-infrastructure --features tcp-runtime,vendored --offline
cargo test -p essim-infrastructure --features tcp-runtime,vendored --offline --test unit_overrides
cargo check -p essim-infrastructure --features tcp-runtime,vendored --offline
cargo check -p essim-workbench --offline
pnpm --filter @essim/workbench test
pnpm --filter @essim/workbench build
```

- 开始基线：core 89 项（含文档测试）、前端 98 项通过。
- 当前 core 104 项通过，其中覆盖专属 15 项。新能力初次行为测试为 4 失败、1 通过，再实现。核心新增覆盖联合创建原子性、同模型不同有效定义、精确初值、禁用引用、悬空补丁、隐藏清理、选择性回写、依赖字段、共享冲突、预览竞态、仓储提交失败、另存／复制身份及快照共享。
- infrastructure 全量 78 项通过，随后新增覆盖专属 3 项通过：展平导出／快照独立、超预算原子拒绝及额度释放、批量应用使用有效表并重校验覆盖。原有空点表可启动用例按新提案改为 `EMPTY_UNIT_MODEL` 拒绝启动，不创建值或占用监听端口。
- 前端 20 文件、110 项通过，包括同值覆盖保留、字段恢复／撤销、隐藏点顺序、基础变化重继承／冲突保留、整端点保存、异步保存期间输入保护。同期属性栏任务新增的测试包含在总数中。
- 构建及 Tauri 检查通过；Vite 仍报告既有大 chunk 提示，不把构建成功解释为大表性能通过。
- 原生发现回写局部点按 PointId 排序的问题，补[失败证据](assets/unit-overrides/order-red.log)后修正为按局部点保存顺序追加；对应测试已通过。

汇总：[测试计数](assets/unit-overrides/test-summary.json)。socket／PTY 测试在允许本机 I/O 的执行环境运行；初次受限环境中的权限拒绝不记作产品通过。

## 独立协议验证

```sh
cargo build -p essim-infrastructure --features tcp-runtime,vendored --example unit_overrides --offline
python scripts/verify_unit_overrides.py
```

Python 使用隔离环境，依赖为仓库 `example/requirements.txt` 指定的 PyModbus 3.11.3、pyserial 3.5。Rust [验证入口](../../crates/infrastructure/examples/unit_overrides.rs)只提供显式命令，独立 Python 客户端／服务端完成实际通信；测试地址 100／110／112、值 12／20／30／33／40／42／55 为零基软件测试约定，无设备资料含义。

| 链路 | 验证结果 |
| --- | --- |
| TCP 从站 | 双 Unit 共享模型、覆盖地址及初值不同；本地确认值捕获；保存／回写不立即生效；应用保留兼容值；复位取已应用有效初值；其他 Unit 值隔离 |
| TCP 主站 | 独立 PyModbus 从站；覆盖地址和 u32 类型读到 42，另一 Unit 读到 7；改映射后失效并读到 55；捕获／保存／回写／复位未产生写请求 |
| RTU 从站 | PTY 双端转发，单 Unit、19200／N／8／2；同样验证有效初值、保存／应用／复位与停机释放 |
| RTU 主站 | PTY 上独立 PyModbus 服务端；双目标 Unit、类型／地址读取、映射失效和无隐式写入通过 |

四组均通过。主站每组捕获 3 个请求，功能码全部为 FC03；另测未读／断开过期值拒绝捕获，从站映射待应用拒绝捕获。操作视图完成与后台归还门闩有短暂差距，驱动器只对明确未受理的 `ENDPOINT_BUSY` 有界重试，不重发已受理命令。原始输出：[interop.log](assets/unit-overrides/interop.log)。

## 原生工作台

构建独立验证包 `ESSim Overrides Check.app`，标识 `dev.essim.overridescheck`，使用 macOS 系统 WebView，没有复用其他工作会话。

验证了中文名称和键盘新建模型选择；直接创建→40 行 TSV 粘贴→保存局部点→回写模型→显式应用；单字段修改、全部恢复及撤销、另存独立模型并绑定；双 Unit 切换保留草稿和字段隔离；关闭时保存完整端点并重开；运行视图及默认初值捕获预览。保存40行时跳过40个不适用的 u16 字序单元格，剩余字段校验成功。填充柄拖动与越界自动滚动、三次撤销回到保存状态已实测。

原生验证修正了旧工具栏27px图标宽度挤压新增文本按钮的问题。局部点回写顺序问题通过核心失败用例定位并修复。截图：[配置](assets/unit-overrides/configuration.png)、[填充与自动滚动](assets/unit-overrides/fill-scroll.png)、[关闭保护](assets/unit-overrides/close-guard.png)、[确认值捕获](assets/unit-overrides/capture.png)。部分截图来自修复排序前的验证会话，不作为最终点顺序证据；最终构建已复查三点回写仍按“总电压、总电流、荷电状态”排列，见[顺序证据](assets/unit-overrides/final-order.png)、[最终覆盖界面](assets/unit-overrides/final-configuration.png)。通过原生导出菜单保存并解析[有效模型 JSON](assets/unit-overrides/effective-model.json)，确认模型格式保持不变、导出地址为覆盖后的110，点顺序保持不变。

中文验证为中文粘贴及展示，没有自动化真实输入法候选窗组合输入；不同输入法、Windows／Linux WebView 和真实 RS-485 硬件未验证。菜单使用已有可访问弹层入口，完整硬件／全平台 UI 验收仍保持独立。

## 代表性内存与耗时

同一 macOS 26.5.2 arm64 主机，Rust 1.99.1，release 无界面构建。每个场景独立进程：1端点、8 Unit、共享1个基础模型、每 Unit 10000点（共80000有效点）、0网络连接，无持续采集或实时刷新负载。覆盖字段为初始工程值，每 Unit 的覆盖值不同。单次应用预览用 `Instant` 测量；目录工程预算为序列化估算；应用后 RSS 用 `ps` 采样，不含 WebView，不是进程峰值。每场景仅一次，不报告统计分位数或吞吐容量。

```sh
cargo build -p essim-infrastructure --example unit_overrides_scale --release --offline
target/release/examples/unit_overrides_scale 0
target/release/examples/unit_overrides_scale 10
target/release/examples/unit_overrides_scale 10000
```

| 每 Unit 覆盖点数 | 应用预览 ms | 应用定义份数 | 目录工程估算 MiB | 采样 RSS MiB |
| --- | ---: | ---: | ---: | ---: |
| 0 | 518.71 | 1 | 34.67 | 163.38 |
| 10 | 524.79 | 8 | 112.30 | 175.17 |
| 10000 | 1091.79 | 8 | 134.19 | 532.53 |

原始结果：[无覆盖](assets/unit-overrides/scale-0.json)、[少量覆盖](assets/unit-overrides/scale-10.json)、[全部覆盖](assets/unit-overrides/scale-10000.json)。同内容共享有效定义；不同有效内容按 Unit 保存完整快照，因此即使少量覆盖也可能增加完整定义份数。稀疏配置不意味着稀疏运行快照。大量覆盖的内存及预览耗时仍有优化空间，不能外推一亿点目标。

配置仍只保存本次会话。M08持久化、模型继承链、配置历史版本、真实 EMS 与硬件验证均不属于本次交付。
