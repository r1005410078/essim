## Why

当前右侧属性范围挤在搜索框旁，与确认的右边缘菜单设计不一致。将范围移入 Dockview 原生右边缘标签，释放搜索宽度并保持工程工作台结构。

## What Changes

- 右边缘组提供点、Unit、端点三个真实 Dockview 标签，只显示图标，名称通过提示与无障碍标注提供。
- 删除内容区横向范围按钮，各标签保留共享 PropertyPanel 与对应属性内容。
- 顶部右面板开关完全隐藏／恢复整个组，保持选中标签与业务草稿。
- 右侧属性采用 Blender 参考的紧凑模式：28px 图标栏、选中底色与内容融合、字段分组压缩；其他布局、配置保存与运行行为保持原样。

## Capabilities

### New Capabilities
- `right-edge-inspector-tabs`: 右边缘图标标签、属性切换及可见性行为。

### Modified Capabilities
无。

## Impact

影响前端 Dockview 组装、属性范围显示及局部样式；不新增依赖，不修改 Rust 或配置格式。复用公共属性组件。
