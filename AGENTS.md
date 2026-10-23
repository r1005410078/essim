# ESSim — 储能设备协议仿真与测试平台

## 开始工作前

先阅读 [项目目标与使用场景](docs/project-goals.md) 和 [README](README.md)。协议学习任务再阅读 [学习路线](docs/learning-roadmap.md)。以用户最新明确指示为准，区分已实现能力、产品计划和教学实验。

本项目有三个目标：接入真实 EMS 做协议与规模测试；学习并熟悉协议；形成可演示、可验证的个人作品。不要把它做成只展示假数据的界面，也不要为了学习而强制重写整个工业协议栈。

## Modbus 实现计划

正式 Modbus 模拟器建设按[实现计划](docs/plans/modbus-simulator.md)推进。相关工作开始前阅读该计划，按大类逐条讨论后形成对应提案；总计划不替代各提案的具体任务与验收条件。

推进时同步维护计划中的讨论、提案、实现、验收勾选项及关联链接。只有已完成的阶段才打勾；大类总览须在约定范围实现并验收后勾选，不能将教学闭环、原型展示或提案完成当作正式功能交付。需求与范围变化同步更新计划、关联提案及项目目标，保留已有验收证据的适用边界。

## 行为准则

参考 SCADA Studio 的“先想清楚、简单优先、手术式修改、测试驱动”原则，按本项目实际情况执行：

1. **先明确成功条件**：多步骤任务先说明简要计划及验证点。缺少协议规则时查规范、设备资料或官方库文档，不猜地址、字序、类型和控制语义。常规实现选择自主推进；影响范围的歧义需说明并澄清。
2. **简单优先**：只实现当前阶段需要的能力，不提前建设通用插件平台、微服务集群或全部协议功能。
3. **手术式修改**：保留用户现有改动，只修改任务涉及的文件。不顺手迁移、删除 Rust 学习代码，不改其他项目来接入组件。
4. **有意义的测试驱动**：协议逻辑、状态机、校验和 bug 修复优先先写失败用例再实现；重构先确认原有测试。文档和低影响样式修改不强制新增测试。不能只写镜像实现的断言。
5. **如实报告**：说明实际变更、验证结果及未验证范围。文档不能把计划写成已完成，不能虚构性能或互操作结果。

## 技术与范围约束

- 正式产品前端使用 **TypeScript + React + Vite**，采用 pnpm workspace、Turborepo、shadcn/ui 和 Tailwind CSS。`prototype/` 保留 JavaScript 原型，不强制迁移。
- 产品方案暂定为 **Rust + Tauri + React 桌面应用**，这是用户确认的最新方向。旧根目录 Rust 学习原型已移除，`workbench/` 和 `crates/` 为正式工程骨架，不宣称已实现协议核心；不要自行切回 Qt/C++ 或 Electron。
- 协议库通过独立 Rust 安全封装接入，明确 unsafe 边界、句柄所有权、线程要求及回调生命周期；C 侧回调不能让 Rust panic 跨越 ABI。仿真核心不依赖 Tauri 窗口，保留无界面运行能力。
- Tauri 使用系统 WebView；已有点表组件必须在目标平台验证填充柄、自动滚动、粘贴、中文输入及大表性能。
- 优先评估 **libmodbus / lib60870 / libiec61850**；调用前核实官方 API、版本、许可和目标平台。不要只为理解协议而在产品路径重写成熟库。
- 学习实验可以手写小范围编解码，必须明确实验覆盖范围，与产品实现保持清晰边界。
- 复用用户已有 Excel 风格组件，先核实位置、导出接口和能力，不能默认它已经在本仓库，也不能自行重写同等组件。
- 本仓库 `prototype/` 已建立 React/Vite/Tauri 原型工具链；实施前检查真实配置，不复制其他项目的包管理器、命令或目录约定。
- 不因为参考仓库使用 OpenSpec、Bun、Storybook 就直接引入；需要时依据本项目规模和用户要求决定。

- 界面使用 **Dockview 停靠布局**，采用紧凑工程工作台风格，不回退到卡片式管理页面。设备维度使用点表库，以及设备 / 通讯 / 采集表引用文件树；策略维度使用脚本编辑与仿真。
- 属性面板统一使用 `@essim/ui/components/property-panel`，新增字段和协议按[属性面板接入规范](docs/design/property-panel.md)组合扩展。布局与控件密度由共享组件维护，业务状态、校验、保存与应用留在应用层，不复制旧面板样式或另建同类表单框架。

## 公共弹框使用规范

- 正式客户端新增或修改业务弹框（包括确认、创建、编辑、导入和配置预览）必须统一使用 `@essim/ui/components/workbench-dialog` 导出的 `WorkbenchDialog`、`DialogBody`、`DialogFooter`，遵循[工程紧凑弹框规范](docs/design/workbench-dialog.md)。不得自行拼装原生 `<dialog>`、遮罩弹层，或使用 `window.alert`／`window.confirm`／`window.prompt` 替代业务弹框。系统文件选择、保存路径窗口保持操作系统提供的界面；菜单、下拉列表和提示使用各自组件。
- 标题通过 `title` 传入，内容放在 `DialogBody`，操作按钮放在 `DialogFooter`，最后一个按钮为主操作。表单使用同一个 `<form onSubmit>` 包住内容与操作栏；取消按钮设为 `type="button"`，保留 Enter 提交与原生字段校验。
- 由父级条件渲染控制打开和关闭，统一通过 `onCancel` 处理取消；业务层不再调用 `showModal()`。异步请求期间传入 `busy`，同时禁用取消／提交按钮，并保留任务取消、失效响应处理及资源清理。
- 拖动、八方向缩放、复位、视口边界、焦点限制及亮色紧凑样式由公共组件维护。业务层仅按需设置 `initialWidth`，不得另加外层 padding、overflow、固定最大高度或复制同类弹框样式。弹框内下拉菜单的 portal 应挂到最近的原生 dialog，避免被滚动区裁切或落入背景层。
- 新增或调整弹框后，按改动验证初始焦点、Tab／Shift+Tab、Esc、关闭后焦点恢复、表单校验、忙碌关闭保护，以及拖动／缩放和窄窗口下内容滚动、操作栏可见性。在工作台实际业务弹框中验证，覆盖窄窗口场景；涉及 WebView 行为时在目标 Tauri 平台复核。

## 架构约束

1. 设备类型、通信角色、协议实例分别建模；设备状态与协议映射分开，同一设备可暴露多种协议。
2. 配置与运行状态分开；点定义使用稳定 ID，不用表格行号作为身份。
3. 区分运行值修改与点表结构修改。应用配置前批量校验；明确保存与应用的生效范围和失败行为，不能留下半应用状态。
4. 协议回调、配置编辑、仿真调度之间明确状态所有权和同步机制。协议线程不能直接操作前端或无同步修改共享配置。
5. C 库封装使用 RAII（Rust Drop），明确句柄释放、回调对象生命周期及停机顺序；异常不得跨越 C ABI 边界。一个设备的断连或异常不应阻塞其他设备。
6. 仿真引擎独立于页面；实时消息按需订阅和批量发送，高频点变化不逐条写配置数据库。
7. 故障注入标明层次。真实网络故障与应用级异常分开，不随意丢弃 TCP 字节来模拟 IP 丢包。
8. IEC 61850 必须声明具体支持的模型和服务；MMS、GOOSE、SV 分阶段处理，不将全部标准族当成普通寄存器表。

## 验证要求

- 按改动运行适当的构建、测试和检查；只使用仓库真实存在的命令。工具链未建立时记录限制，不编造通过结果。
- 协议解析覆盖分段/合并接收、非法长度和异常响应；控制逻辑覆盖拒绝、超时及状态变化等实际相关分支。
- 对外通信必须逐步增加独立互操作证据；自己的客户端与服务端互测不能作为完整兼容性证明。
- 规模报告必须注明环境、设备/连接/点数量、数据频率、持续时间及测量方法，不用“高性能”“支持海量数据”代替数字。
- 学习实验记录假设、原始报文、预期与实际结果、故障定位及覆盖边界。

## Rust 注释规范

- 遵循 rustdoc 约定，正文使用中文，保留准确的 Rust 和协议术语；章节名使用 `# Examples`、`# Errors`、`# Panics`、`# Safety` 等标准英文名称。
- `//!` 描述当前 crate 或模块，通常放在文件开头；`///` 描述紧随其后的公开 API，包括类型、字段、枚举变体、函数、常量和 FFI 声明；`//` 用于解释实现原因、设计取舍和局部约束。
- 公开 API 的文档先用一句话说明用途，再按实际需要补充参数含义、单位、地址基准、返回值、所有权和生命周期；不重复类型签名，不为凑格式添加无内容的章节。
- 存在错误返回时用 `# Errors` 说明失败条件；可能 panic 时用 `# Panics` 说明触发条件；需要展示用法时提供 `# Examples`。可执行的 Rust 文档示例应保持可通过文档测试，不用 `ignore` 掩盖示例失效。
- 每个公开 unsafe 函数及 unsafe FFI 函数声明必须有 `# Safety`，明确调用者必须保证的条件，例如指针有效性、缓冲区长度与对齐、对象生命周期、独占访问及线程要求。模块级说明不能代替各函数的具体安全契约。
- 每个 unsafe 块前写 `// SAFETY:`，解释这一次操作为何满足相应安全条件，依据应来自实际所有权、校验或同步机制；不能仅写“调用 C 函数”或“这里是安全的”。`unsafe impl` 同样需要说明安全依据。
- FFI 注释还须说明句柄的创建与释放配对、socket 等外部资源的管理责任、返回码和 errno（适用时），并区分上游库 API 与本项目辅助函数。第三方 vendored 源码保持原样，不为统一注释改写。
- 注释随实现同步更新。文档改动按需运行 `cargo doc`，新增或修改可执行示例时运行对应包的 `cargo test --doc`；构建参数遵循各包实际 feature 和依赖要求。

## 文档与提交

- 目标或范围变化同步更新 `docs/project-goals.md`；入口或实现状态变化同步更新 README，学习进度变化更新对应学习文档。
- 文档使用中文，保留准确的协议术语；示例地址和数据必须注明为教学约定或资料来源。
- 提交时使用中文 Conventional Commits，例如 `feat(modbus): 增加多设备点表映射`；只有用户要求提交时才执行提交。

<!-- BEGIN:turborepo-agent-rules -->

# This is NOT the Turborepo you know

Turborepo configuration, task behavior, and CLI commands can vary between installed versions and may differ from your training data. Resolve the `turbo` package from this file's directory or relevant workspace; in monorepos, it may not be visible from the repository root. For example, run `node -p "require.resolve('turbo/package.json')"` from a workspace that depends on `turbo`.

Read `docs/README.md` inside that installed package first, then read the relevant pages from its `docs/` directory before changing Turborepo configuration or commands. Heed deprecation notices. These bundled docs match the installed package version and are available without network access.

This block is written and re-added by `turbo` before repository-scoped commands when an AI agent is detected. In the Turborepo source repository, its template is defined in `crates/turborepo-cli/src/cli/agent_guidance.rs`. Removing the managed block while updates are enabled means a later qualifying invocation will add it again. Set `"agentGuidance": false` in the root `turbo.json` or `turbo.jsonc` to opt out; this does not remove an existing block. Keep the block committed with your work to avoid an uncommitted change on the next agent invocation.
<!-- END:turborepo-agent-rules -->

## 配置简化约束

模型与端点不提供配置版本管理。每个模型仅保存当前内容，单元通过稳定ID引用；保存覆盖当前内容，显式应用后更新独立快照。不重新引入历史版本、选版、升级或expectedRevision。内部通知序号与文件格式标记不属于配置版本功能。

## 正式工程约束

正式客户端位于 `workbench/`，Rust 使用根 Cargo workspace。核心 `crates/core` 不依赖 Tauri，基础设施位于 `crates/infrastructure`；前端业务组件通过统一接口访问后端，Tauri API 仅允许在应用组装与通信适配层使用。后续独立服务通过新增适配器接入，当前不建 Qt 界面、不做策略。
