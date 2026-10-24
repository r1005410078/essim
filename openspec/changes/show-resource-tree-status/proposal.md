## Why

左侧树缺少运行反馈，用户必须逐个打开端点确认状态。按已确认设计在节点最右侧显示状态图标，保留紧凑树布局。

## What Changes

- 端点运行状态、Unit禁用／采集暂停状态及独立配置标记采用右对齐彩色图标，hover显示说明和真实错误。
- 查询只覆盖当前可见树节点对应端点，使用空点列表读取运行元数据，不读取全部点值，不触发启停。
- 保留节点图标、名称、展开、选择和右键菜单。监听不推导Unit在线；未知状态明确提示。

## Capabilities

### New Capabilities
- `resource-tree-status`: 资源树的真实运行状态与配置状态展示。

### Modified Capabilities

无。

## Impact

涉及ProjectNavigation、状态映射和可见节点元数据查询，以及运行视图已观察质量记录；复用现有API和Tooltip，不新增依赖或协议操作。
