# 公共 MarkdownTooltip 与箭头修复

日期：2026-10-23。

## 变更

公共 UI 包增加 MarkdownTooltip，统一正文、可选来源、GFM、跳过原始HTML、紧凑排版、可用高度内滚动、窄窗口退让和 dialog portal 容器。数据区选项与 FieldHelp 已迁移，业务层重复样式移除。UI 包使用项目已有 Markdown 依赖版本，锁文件及链接通过离线安装校验。

基础 Tooltip 的 Radix SVG 原本叠加方块背景、45度旋转及位移，导致菱形。移除这些额外样式，恢复10×5原生三角箭头；Markdown箭头与浮层背景同色，普通提示仍使用深色背景对应颜色。外层不滚动，箭头不被长文区域裁切。

## 验证

- `pnpm --filter @essim/ui typecheck`：通过。
- `pnpm --filter @essim/workbench test`：27个文件、126项通过。新增公共正文测试覆盖GFM标题、加粗、代码、表格、来源可选与原始HTML跳过。
- `pnpm --filter @essim/workbench build`：通过，保留已有大块体积提示。
- 浏览器临时夹具挂载真实公共组件：检查上、右、下、左实际定位及原生三角 polygon；Markdown箭头与白色浮层均为 `rgb(255,255,255)`，普通提示箭头与背景均为 `rgb(36,49,64)`。
- FieldHelp的真实GFM表格正常展示；长文100项列表内层 `clientHeight=347`、`scrollHeight=1980`、`overflow=auto`，外层 `overflow=visible`；Esc关闭后焦点仍在触发按钮。
- 360×640窄视口：浮层向下展开，左边界8px、宽340px，完整位于视口内。预览后恢复视口并清理临时夹具。
- [修复后截图](2026-10-23-markdown-tooltip.png)。

## 未验证范围

本轮未进行 Tauri 系统WebView与真实通信复测，未将 dialog 内 portal 行为计为原生验收。M02总体验收状态不变。
