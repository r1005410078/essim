# 移除配置版本管理（2026-10-16）

用户要求简化模型与端点配置，取消全部用户可见配置版本管理。该决定取代M01／M02此前的不可变模型版本和选版规则；旧验收记录只保留历史适用边界。

## 实际变更

- 模型目录按稳定ID只保留一份当前内容，保存覆盖原对象，复制生成独立模型／点ID。
- CollectionModel、ModelRef、EndpointConfig、RunSnapshot、命令和确认DTO移除保存版本、配置修订及expectedRevision；删除版本冲突检查和模型历史保留端口。
- 单元只引用模型ID；UnitValues保留已应用的完整模型快照。覆盖保存不改变已应用快照和值，显式应用时整组校验后替换。停机期间权威内容变化仍拒绝覆盖，失败不半应用。
- 删除模型仍校验配置／已应用引用；未来运行记录携带自包含快照，不依赖模型历史版本集合。
- 工具栏、Details、复制和导出移除选版、版本列表、已应用版本、升级提示及版本文件名。只显示保存／应用状态，待应用按内容比较。
- 文件formatVersion、事件schema和通知sequence仍为内部格式与传输标识，不提供配置版本管理功能。草稿编辑计数仅用于刷新与拒绝过期导入。

## 自动化验证

- `pnpm --filter @essim/workbench test`：36项通过，包含不携带版本保存、保存等待期间新编辑保护，以及模型覆盖后已应用状态比较。
- `pnpm --filter @essim/workbench build`：类型检查与生产构建通过，保留已有大chunk提示。
- `cargo test -p essim-core -p essim-infrastructure --offline`：42项集成测试及1项文档测试通过；覆盖单份模型、独立复制、引用保护、保存不改变应用快照、显式应用迁移、主站失效规则及三格式交换。
- `cargo build -p essim-workbench --offline`：通过。
- `cargo doc -p essim-core -p essim-infrastructure --no-deps --offline`：通过。
- `cargo test --workspace --offline`：未完成；未改动的libmodbus-sys构建所需autoconf在本机工具路径不存在。本次未修改该协议库构建规则，不能记为全workspace通过。

## 原生验证及边界

使用最新桌面程序的独立macOS Tauri测试窗口，创建“单份配置验证”、新增点并保存，再改名为“单份配置覆盖保存”并再次保存。界面正确从未保存变为已保存，资源树仅保留一个模型；工具栏无导出版本选择，Details只展示模型引用。未以此宣称真实EMS通信、Linux／Windows、SQLite、可靠录制或完整M02人工验收完成。复制、导出保真和应用隔离由上述自动化测试覆盖，未在本轮重复全部原生文件对话框操作。
