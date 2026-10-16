# M01 基础模型实施记录

M01约定的基础模型、契约、清理与配置工作台范围已完成验证；完整模拟器仍按后续阶段交付。环境：macOS arm64；日期2026-10-15。当前结果不能替代真实EMS、真实串口／RS-485、多平台或规模验证。

## 基线与保留边界

改动前 `cargo test --workspace --offline`、`pnpm --filter @essim/workbench test`、`pnpm --filter @essim/workbench build` 通过，原始输出见 [Rust基线](m01/before-rust.txt)、[前端基线](m01/before-ui.txt)。前端构建存在大chunk提示，不作为性能通过证据。

保留 `crates/libmodbus/` 三包、官方源码／许可／通用测试、`scripts/verify_modbus_master.py`、`prototype/`、NovaSheet vendor、共享UI、打包工具、所有历史记录及既有提案。`extract-libmodbus-workspace` 的真实RS-485任务9.9继续未验；`integrate-libmodbus-tcp-loop` 已有历史验收不视为新系统验收。

待替换入口：core旧modbus／simulator、infrastructure固定SOC worker／Native、Tauri旧教学命令／快照、前端SimulatorPanel／SOC表格。现有 commands 文件拆分与中文说明保留职责结构，不用git restore覆盖未提交修改。新树使用 `@pierre/trees@1.0.0-beta.6`，点表继续复用NovaSheet。

## 测试／脚本迁移索引

| 原位置／用例 | 本阶段处理与后续义务 |
| --- | --- |
| core/modbus_rules::soc_validation_and_initial_value_are_separate | 固定SOC范围断言退役；foundation::migration_keeps_raw_across_scale_and_address_changes_and_is_atomic覆盖通用值迁移；M02完整类型编码另验 |
| core/modbus_rules::lifecycle_rejects_duplicate_start_and_stop_is_idempotent | runtime_contracts::stop_requires_confirmation_reconnect_and_pause_keep_run_and_restart_needs_new_id；显式确认资源释放 |
| core/modbus_rules::endpoint_rejects_invalid_addresses_and_ports | EndpointConfig::validate基础参数校验；foundation新增非法参数测试；真实打开失败M04/M05重验 |
| tcp_loop::real_read_update_policy_and_lifecycle | SOC及外部只读教学策略退役；M04按通用点表重做四区读取／写权限／异常码与独立互操作 |
| tcp_loop::stream_boundaries_bad_headers_extra_client_and_stop | M04必须恢复分段合并、坏MBAP、非法长度、额外连接及停机；M01契约测试不替代真实I/O |
| tcp_loop::bind_failure_recovers_unit_id_and_malformed_length_close | M04必须恢复端口占用后的恢复、未知单元及非法长度关闭测试 |
| tcp_loop::subscriptions_are_bounded_and_disconnected_receivers_reclaimed | 新Tauri订阅保留上限、取消和回收；M07高频批量背压另验 |
| tcp_loop::nonreading_client_cannot_block_stop | M04/M06必须恢复不读对端下有界停止实测；纯状态机不证明socket期限 |
| tcp_loop::units_can_be_added_live_and_keep_independent_values_after_restart | foundation覆盖稳定身份、单元独立模型及应用值；M04动态运行配置／停启值保留重验 |
| rtu_loop::serial_read_update_readonly_broadcast_mode_isolation_and_restart | 教学SOC与模式切换入口退役；M04恢复通用RTU映射、广播、独立值、重启和独立PyModbus互操作 |
| rtu_loop::missing_device_fails_and_configuration_is_atomic | foundation覆盖配置原子性；M04恢复实际设备不存在／打开失败 |
| rtu_loop::serial_disconnect_does_not_block_stop | M04恢复断开PTY／串口后停止；真实硬件仍待验 |
| rtu_loop::slow_maximum_frame_and_continuous_bad_crc_have_bounded_stop | M04/M06恢复最大慢帧／连续CRC故障下停机实测 |
| frontend contracts::旧序号／旧代次过滤 | 替换为完整目录快照字符串序号过滤；无固定demo实例 |
| frontend client::浏览器无后端 | 保留明确失败，不返回假目录或启动成功 |
| frontend tauri旧4条教学命令测试 | 替换为通用配置命令、失败传播及幂等取消；旧SOC／RTU专用命令退役 |
| frontend unitConfig／rtuConfig测试 | 基础参数移到核心聚合；完整RTU编辑器与超时M03/M04另验 |
| frontend socSheet旧3条草稿测试 | SheetViews和WorkspaceStore覆盖按稳定Unit／分类隔离、独立草稿／已应用状态 |
| scripts/verify_modbus.py | SOC地址100客户端断言退役；M04新的独立从站验证补回 |
| scripts/verify_headless.py | 旧stdin SOC接口退役；M04通用无界面运行与资源释放重验 |
| scripts/verify_rtu.py | 旧SOC PTY从站验收脚本退役；M04恢复通用从站软件及硬件验证 |
| scripts/verify_modbus_master.py、libmodbus三包tests | 原样保留，静态／显式动态模式及独立PyModbus TCP／PTY互操作均需复验 |

新增不变量先失败的输出见 [身份模型红灯](m01/red-foundation.txt)、[运行门禁红灯](m01/red-runtime.txt)。测试替身只在测试代码中；不宣称可靠落盘或实际协议已完成。

## 交付结果与验证

- `cargo test --workspace --offline`：41个单元／集成测试及9个文档测试通过，见[最终Rust输出](m01/after-rust.txt)。其中16个core配置／运行契约测试和1个Tauri订阅销毁测试；测试替身仅在测试文件。与原37个单元／集成测试相比，删除13个教学用例，新增17个用例；数量变化不能解释为协议覆盖增长。
- 显式动态前缀 `target/libmodbus-shared/aarch64-apple-darwin-Dynamic-debug/install`，`cargo test -p libmodbus-rs -p libmodbus-sys -p libmodbus-src --no-default-features --offline`：保留通用库24个单元／集成测试及9个文档测试通过，见[动态模式](m01/dynamic-library.txt)。
- 分别构建静态／显式动态 `client_demo` 后，以本机Python环境PyModbus3.11.3／pyserial3.5运行`verify_modbus_master.py`：独立TCP与PTY RTU从站四区读取、单／多写、异常后继续请求均通过，见[静态互操作](m01/interop-static.txt)、[动态互操作](m01/interop-dynamic.txt)。每种传输14次测试请求，地址与数值为脚本测试约定；无真实串口电气或RS-485证据。
- `cargo doc --workspace --no-deps --offline`通过，见[文档构建](m01/rustdoc.txt)；core／infrastructure额外以`RUSTDOCFLAGS='-D warnings'`构建通过。`cargo tree -p essim-core --offline`仅serde及其派生依赖，无Tauri或libmodbus。
- 前端11个行为测试通过，覆盖无后端拒绝、命令失败传播、取消幂等、精确序号、树稳定ID映射、Unit不发命令、独立草稿、确认值刷新、保存期间继续编辑及关闭欢迎页后的文档定位。`pnpm --filter @essim/workbench build`包含TypeScript检查并通过；Vite仍提示大chunk，未作大表或加载性能承诺。
- `cargo fmt --all -- --check`、触及的TypeScript／CSS使用固定Prettier3.6.2格式化、`git diff --check`和OpenSpec严格校验通过。
- `python3 scripts/bundle_macos.py static`构建本机调试包并通过ad-hoc签名校验，见[打包输出](m01/bundle.txt)。当前桌面应用不链接libmodbus；打包脚本按实际依赖处理，不复制无用动态库或虚称应用通过两种原生链接验收。没有公证或公开分发承诺。

部分配置用例在实现后补齐，并非所有任务都严格按“先失败再实现”顺序完成；身份模型、运行门禁、资源占用及保存竞态有保留的失败输出。配置行为的实际验收依据为上述测试，不虚构测试执行顺序。

## 目标WebView操作验收

使用macOS系统WebView运行真实Tauri应用包（1440×980逻辑窗口），通过原生UI操作：

1. 空项目打开，无设备、点值和通信历史；核心连接与会话存储限制可见。见[初始空态](m01/desktop-empty.png)。
2. 新建空模型、TCP从站端点，在属性栏添加Unit1。未保存时应用禁用；保存后仍未应用；显式应用后显示已应用版本，通信状态持续未运行。
3. 添加Unit2并保存／应用；Unit2选择遥信，切回Unit1仍选遥测，从Pierre树返回Unit2仍是遥信。见[多单元操作截图](m01/desktop-applied-units.png)。该图为本轮中间版本，后续已去掉树标签UUID和表格默认插画。
4. 新建RTU从站配置，路径`/dev/m01-validation-only`为验收占位，并未打开串口；添加一个Unit后继续添加禁用。纯配置可保存，真实启动仍禁用。
5. 更多中的置数、复位、导出、录制、回放显示未接入；未向测试服务器发送任何产品运行请求。
6. 正常退出再启动最终签名调试包，原配置清空，验证本次会话边界。见[重启空态](m01/desktop-restarted-empty.png)；同时Rust测试验证Subscriptions销毁逐一取消并join全部推送线程。
7. 最终包再次走通模型→端点→Unit→保存→应用；保存模型v2后，单元仍配置引用／应用v1并提示新版本v2。创建PCS资源、关联单元并保存，设备树快捷入口打开同一端点Tab，没有新建实例。见[最终界面](m01/desktop-final.png)：端点已保存v3、已应用v2，模型仍引用／应用v1，新模型v2可选，保存与应用状态分离。

默认生产目录仍为空；截图中的名称和配置均由验收操作手动创建，不是启动预置数据。没有用假Recorder或假Executor让生产按钮成功。

## 本轮明确未验／未交付

- M02完整点定义编辑、导入／导出、字序／编码转换、地址占用检查；当前只创建空模型及保存版本。NovaSheet在WebView显示和空分类切换通过，非空表选区／滚动由组件保留与数据源测试支持；填充柄、自动滚动、Excel多格粘贴、中文输入及大表性能尚需M02专门验收。
- M03完整CRUD／删除、任意文件夹整理和串口全参数编辑；树重命名／移动未提供，映射测试不当作这些操作交付。
- M04/M05生产TCP／RTU多实例执行器、真实主从运行、采集／置数和协议资源停止期限；旧教学协议回归按迁移索引恢复。
- M06规模与资源调度；当前保留已访问单元／分类的Grid以保持视图，未证明大量活跃表格或设备下的内存、CPU性能。
- M08可靠落盘、崩溃恢复、容量与数据库；M09可用历史加载和回放UI。运行／录制／回放核心测试不能替代这些生产能力。
- 真实EMS、真实RS-485、Linux／Windows及公开分发均未验。旧任务9.9未勾选，旧提案未自动归档；本次未执行Git提交或远程推送。

后续同日按用户截图完成[v15样式对齐](2026-10-15-m01-v15-style.md)，当前界面以该记录截图为准，基础功能和未交付范围不变。
