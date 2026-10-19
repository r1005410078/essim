## Why
工作台配置动作、运行操作和原始诊断混在一起，底部通信报文仍为空壳。用户已确认全局整理方案，并要求表格继续遵循 NovaSheet。

## What Changes
- 统一紧凑工具栏、状态层次、项目导航和上下文属性组织。
- 明确保存配置、应用配置、提交运行值三个动作；整理应用预览。
- 底部接入真实通信与事件，支持筛选、暂停和导出；移除表格上方原始 JSON 和未接入占位工具。
- 模型字段帮助按需展开；保留 NovaSheet 原有样式与交互。

## Capabilities
### New Capabilities
- `workbench-presentation`: 工作台信息层次与配置流程。
- `runtime-observation-panel`: 真实报文和运行事件展示。
### Modified Capabilities
无。

## Impact
涉及 workbench 外围 UI、运行查询观测 DTO、基础设施记录投影及测试文档。不增加协议功能、数据库或配置版本管理，不代表 M07 整体交付。
