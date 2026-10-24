# 属性面板组件与接入规范

正式工作台属性面板统一使用 `@essim/ui/components/property-panel`。结构参考 [UE Details Panel 的分类与搜索](https://dev.epicgames.com/documentation/en-us/unreal-engine/level-editor-details-panel-in-unreal-engine)，采用 ESSim 浅色主题。共享组件不依赖 Dockview、Tauri、WorkspaceStore 或协议模型。

## 组件边界

| 组件 | 职责与扩展点 |
| --- | --- |
| `PropertyPanel` | 外壳；`title`、`subject`、`icon`、`status`、`actions`、`filters` 插槽；可选受控 `search`；支持根节点 ref |
| `PropertyGroup` | 分类折叠；`title`、`defaultOpen`、`hidden`；原生键盘展开／折叠 |
| `PropertyRow` | 标签和值的双列布局；`label`、`help`、`field`、`keywords`；children 接受业务控件 |
| `PropertyActions` | 值列中的选择器与操作按钮横向排列 |
| `PropertyIconButton` | 有明确名称的图标按钮；`label` 同时提供可访问名称和悬停提示；默认不提交表单 |
| `PropertyScope` | 受控范围筛选按钮组；`value`、`onChange`、`options`；不承载业务导航 |
| `PropertyHint` | 辅助说明；错误可传 `role="alert"`，动态反馈可传 `role="status"` |

组件源码和样式位于 `packages/ui/src/components/property-panel.tsx`、`packages/ui/src/property-panel.css`。应用统一导入 `@essim/ui/styles.css`，不复制面板样式，不依赖旧 `.details`／`.property` 类。

## 接入示例

```tsx
import { useState } from "react";
import { PropertyPanel, PropertyGroup, PropertyRow, PropertyHint } from "@essim/ui/components/property-panel";

function ConnectionProperties({ address, onAddressChange }: {
  address: string;
  onAddressChange: (value: string) => void;
}) {
  const [search, setSearch] = useState("");
  return <PropertyPanel subject="TCP 从站" search={{value: search, onChange: setSearch}}>
    <PropertyGroup title="通信参数">
      <PropertyRow label="监听地址" field="transport" keywords="IP address">
        <input aria-label="监听地址" value={address}
          onChange={event => onAddressChange(event.target.value)}/>
      </PropertyRow>
    </PropertyGroup>
    <PropertyHint>修改进入草稿，保存与应用由业务模块负责。</PropertyHint>
  </PropertyPanel>;
}
```

## 扩展规则

1. 新协议、新对象或新字段通过组合组件接入。控件按 `value/onChange/disabled` 等现有接口访问业务层；组件库不得调用后端、读取运行状态或自动保存／应用。
2. 字段使用稳定业务 ID，`field` 对应已有校验定位键，不能使用表格行号。应用可通过根 ref 定位 `data-config-field`，展开所在分组并聚焦控件。定位前应清空搜索并选择相应范围。
3. `PropertyRow` 的 `label` 必须为可搜索的纯文本；帮助放入 `help`。每个输入、选择器、复合字段子控件都需独立 `aria-label` 或 `aria-labelledby`。行是带名称的 group，不用一个 HTML label 包住多个交互控件。
4. 只读值使用文本；暂不可编辑的输入／选择器使用原生 `disabled`，说明原因。字段错误由业务校验提供，控件设置 `aria-invalid`，错误说明通过 `aria-describedby` 关联；组件不猜协议取值规则。不可为了样式把真实错误藏进 tooltip。
5. 串口发现、模型引用等复合控件继续属于业务模块，通过 children 接入。紧凑单行使用 `PropertyActions`；多行控件在值列内布局，行高随内容增长，不能绝对定位覆盖下一行。
6. 搜索按标签及显式 `keywords` 忽略大小写匹配，忽略首尾空白，不搜索字段值。搜索期间展开分组，无匹配行的分组隐藏；清空后恢复原展开状态。自定义可搜索字段必须使用 `PropertyRow`。搜索空态由面板提供。
7. 范围过滤由应用决定；需要跨范围搜索时，搜索期间取消分组的 `hidden`。范围切换只影响展示，不修改配置、选中对象或运行状态。
8. 头部最多使用两行：标题、当前对象与状态合并为一行；搜索与范围筛选合并为工具行，范围组件通过 `filters` 插槽接入。当前对象长文本单行省略，字符串对象名悬停可读完整内容。无搜索／筛选时不渲染工具行。面板填满父容器并独立滚动；父容器须有确定高度。默认属性行 28px、控件 22px、分类标题 27px、头部标题行 28px，搜索／范围工具行 30px；标签 11px、值 12px。长选择值截断，长说明换行，图标按钮不收缩。280px 窄面板作为当前检查基线。
9. 样式扩展优先使用根节点 CSS 变量 `--property-label-width`（默认面板内容宽度的 36%，常规最小标签列 112px，包含文字内边距）、`--property-row-height`、`--property-control-height`，在同一面板统一设置，不逐字段覆盖通用尺寸。业务专属修饰只放在应用层并限定 `.property-panel` 范围。
10. 名称列与值列之间必须有连续的 1px 纵向分隔线，颜色由 `--property-column-divider` 控制。分隔线属于值单元格边界，覆盖完整行高，不能画在输入控件上或用行间 gap 模拟；只读文本、复选框、下拉、复合字段都保持相同列位置。分类标题、提示和其他通栏内容不画纵线。
11. 列宽由每个已挂载的 `PropertyPanel` 统一持有，业务不逐行存储或同步。拖动任意属性行分隔线使全部分组同步，范围／搜索切换继续使用同一列宽；不同面板相互独立。分隔线具有6px命中区，指针捕获保证移出行后仍能拖动，释放、取消、丢失捕获或行卸载时结束拖动。支持左右方向键每次8px、Shift加速24px，Home或双击恢复CSS默认列宽。
12. 调整后按比例响应面板宽度变化，并由CSS再次限制：默认名称列保留原始最小宽度；用户拖动或键盘向左调整时允许名称列缩到0，不再强制112px最小宽度，字段名逐步省略；值列仍保留可用宽度。列宽仅为当前面板的界面状态，重新挂载后恢复默认，不写入配置文件。

## UE5 调研与本项目取舍（2026-10-22 补充）

此前只缩小控件、保留两列间距，遗漏了属性表的列边界。本次核对 Epic 的 UE5 文档、公开 API，以及[官方教程中的面板截图](https://dev.epicgames.com/documentation/en-us/unreal-engine/designer-05-puzzles-switches-and-cubes-in-unreal-engine)后修正。截图可见名称／值纵线跨属性行对齐，在 Transform、Sockets、Static Mesh、Materials 等通栏标题处中断；缩略图与多行控件仍使用同一列边界，最右侧另有操作区域。这是结构与视觉核对，不是固定像素、主题色或全部功能的复制。

| UE5 中核实的设计 | ESSim 对应规则 |
| --- | --- |
| `FDetailWidgetRow` 分别提供 `NameWidget`、`ValueWidget`、右侧 `ExtensionWidget` 和 `ResetToDefaultWidget`，也允许整行内容 | 名称与值作为独立单元格；分类标题与说明通栏；已有引用操作在值列内组合，不为不存在的功能添加重置按钮 |
| `FDetailColumnSizeData` 独立管理名称／值／右侧列宽及 splitter 悬停、调整回调 | 整个面板统一列宽和分隔线位置；现由共享面板管理同步列宽，提供拖动、键盘调整和恢复默认；右侧独立扩展列仍作为参考 |
| Details Panel 使用分类组织属性，搜索根据输入过滤 | 使用统一分组和搜索；不把每组变成带外边距的卡片 |
| 属性可有编辑条件，已修改值可提供恢复默认值入口 | 禁用状态与只读值区别呈现；恢复行为必须由业务层定义，不根据显示值推断默认值 |

来源：[FDetailWidgetRow](https://dev.epicgames.com/documentation/unreal-engine/API/Editor/PropertyEditor/FDetailWidgetRow)、[FDetailColumnSizeData](https://dev.epicgames.com/documentation/unreal-engine/API/Editor/PropertyEditor/FDetailColumnSizeData)、[UE5.6 Details Panel](https://dev.epicgames.com/documentation/en-us/unreal-engine/level-editor-details-panel-in-unreal-engine?application_version=5.6)、[UE5.6 属性自定义](https://dev.epicgames.com/documentation/en-us/unreal-engine/details-panel-customizations-in-unreal-engine?application_version=5.6)。行高、色值、默认列宽为 ESSim 的适配参数，不是声称从 UE5 提取的固定数值。

当前采用组合组件，不建立字段注册中心、协议插件系统或自动表单引擎。保存／应用、配置／运行值、稳定模型引用规则均保持原业务边界。

## 验证

执行 `pnpm --filter @essim/workbench dev:tauri`，在工作台选择对象后验证对应属性面板。覆盖 340px 和 280px 面板、长选择值、禁用按钮、复合字段、范围筛选、搜索和折叠。独立预览 HTML 入口已移除。

已有接入包括端点／Unit／运行点／设备／文档属性和模型点编辑。验证证据与限制见 [本轮记录](../test-records/2026-10-22-property-panel.md)。

## Dockview 右边缘属性范围（2026-10-23）

工作台右侧使用真实 Dockview right edge，headerPosition 为 left，点／Unit／端点是三个独立标签，内容仍复用公共 PropertyPanel。标签只显示 ListTree／Layers／Server 图标，不在下方放文字；悬停 title 与原生标签 aria-label 提供名称。默认选中 Unit，搜索与折叠状态由各面板保留。上下文为端点时，各标签只展示和搜索自己的范围；设备、采集模型和文档继续使用已有上下文属性内容。

顶部右面板图标通过 Dockview 公开可见性接口隐藏／恢复整个组，隐藏时不保留图标栏；恢复保持标签及尺寸。切换属性标签不写入文档历史，不改变选中 Unit、运行点、配置草稿或运行状态。端点校验根据字段定位 Unit／端点标签。其他布局与初始比例保持。

[提案](../../openspec/changes/use-right-edge-inspector-tabs/proposal.md) · [验证记录](../test-records/2026-10-23-right-edge-inspector-tabs.md)。

模型“选中点”属性与配置表共用 gridFields，均不提供四遥分类和备注编辑项；四遥分类仍由底部分类视图承载。既有数据和交换格式保持，隐藏字段不因界面调整丢失。

位置修订：属性组仍停靠工作台右侧，三个纯图标标签移到属性内容左边；仅移动该组的标签位置和分隔线，其他布局不变。

## Blender 参考紧凑模式（2026-10-23）

右侧属性区启用公共 `PropertyPanel density="compact"`：头部24px、分组头22px、普通字段行22px、控件20px、分组无外边距。字段名左对齐，控件同列等宽，保持UE风格的连续纵向分隔线、细行线和通栏分组；长标签与复合字段允许自然增高。标准与紧凑模式均保留列宽拖动命中区及悬停／焦点提示，Blender参考仅用于外侧图标栏的紧凑和选中开口效果。

右侧图标栏宽28px，图标行高30px；选中整行与相邻内容同为 `#f7f8fa`，无圆角、蓝线、蓝底块或分隔边，未选中栏为 `#e8ebef`。原生键盘焦点提示保留。

模型点按点定义、数值解释、访问与采集组织，编码预览默认折叠；模型名称与引用保持。字段值、保存、应用与运行流程不变。

[紧凑版验证](../test-records/2026-10-23-compact-inspector.md)。

拖动时由公共面板绘制单个1px蓝色指示条，覆盖首个可见字段至末个可见字段，跨过行边框及分组间距；不再由每行拼接蓝线。释放／取消／失去捕获后移除指示条。Home／双击恢复默认列宽。

字段名称列默认宽度为102px；手动拖动与键盘调整仍可改变，Home／双击恢复102px。窄面板继续保留值列宽度约束。
