## Context
已有字节解析函数及shadcn Tooltip，但Tooltip不适合容纳可点击链接；旧箭头叠加旋转和背景产生菱形。

## Goals / Non-Goals
**Goals:** 点击解析、三角箭头、文档分类、同Tab复用、章节定位、离线Markdown。
**Non-Goals:** 文档编辑器、任意本地文件读取、评级、协议执行扩展。

## Decisions
- 使用现有radix-ui依赖组装shadcn Popover，箭头由原生Arrow绘制，不叠加旋转方块。
- 内置文档存放workbench/src/resources/protocols/modbus，注册稳定资源ID与目录路径。资源树与采集模型平级，不写会话配置目录。
- Markdown使用现有react-markdown/remark-gfm，禁用原始HTML，标题生成稳定锚点；目录与文内链接在当前面板滚动，外部链接只允许http(s)。
- “详细”按功能码／异常定位章节；Doc ID不含锚点，因此不同入口复用同一Dockview Tab。

## Risks / Trade-offs
- 静态文档与能力漂移 → 支持范围单独声明并在测试记录中校对。
- 点击入口嵌套行按钮 → 行使用可聚焦row容器，内部字节使用真正button。
- 章节锚点变动 → 自动测试检查注册映射、目录路径和正文锚点。
