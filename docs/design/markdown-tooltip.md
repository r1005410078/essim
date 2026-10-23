# Markdown 说明提示

协议、字段等较长说明统一使用 `@essim/ui/components/markdown-tooltip` 的 `MarkdownTooltip`。短纯文本提示继续使用基础 Tooltip。

```tsx
import { MarkdownTooltip } from "@essim/ui/components/markdown-tooltip";

<MarkdownTooltip
  markdown={helpMarkdown}
  source="Modbus V1.1b3 §4.3、§6"
  side="right"
  align="start"
>
  <button type="button">数据区说明</button>
</MarkdownTooltip>
```

- `children` 为可接收 ref 和焦点／指针事件的单个触发元素，可直接传 SelectItem；不额外嵌套按钮。
- `markdown` 为正文；`source` 为可选来源栏。业务负责内容及协议语义，公共组件负责 GFM 渲染、排版、定位、宽度和滚动。原始 HTML 不参与渲染。
- 默认350ms悬停延迟，键盘聚焦可展开，Esc关闭。默认优先在上方，窄于640px的窗口优先在下方，Radix负责碰撞避让。最大宽度340px，边缘至少留8px；正文最长420px并受实际可用高度约束。
- 自动将 portal 挂到触发元素所在的最近原生 dialog，其他场景挂到 body。
- 内层负责长文滚动，外层保持overflow可见，避免裁切箭头。不要在业务层复制说明样式或重新旋转箭头。
- 基础 Tooltip 的箭头采用 Radix 原生三角形，由 Radix处理方向；Markdown 提示通过共享变量将箭头颜色设为提示框背景色。

当前消费者：点表数据区、类型、字节序和字序下拉选项，以及 FieldHelp。点表与模型属性栏共用 PointSelectItem，业务协议文案留在工作台的 Markdown 文件中。新增说明优先复用此组件。
