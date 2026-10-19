## ADDED Requirements

### Requirement: 点击解析
报文 SHALL 点击或Enter打开解析Popover，悬停不打开；箭头为与面板一致的三角形，支持Escape及外部点击关闭。

#### Scenario: 打开解析
- **WHEN** 用户点击报文字节
- **THEN** 浮层保持可交互，包含字段解析、“详细”和关闭入口

### Requirement: 分类资源文档
客户端 MUST 提供与采集模型平级的文档资源树，按协议／Modbus分类，Markdown作为内置资源离线渲染。

#### Scenario: 从项目树打开
- **WHEN** 用户选择Modbus详细指南
- **THEN** Dockview显示可停靠、可关闭、无编辑草稿的文档Tab

### Requirement: 章节联动
“详细” SHALL 打开当前功能码或异常章节，复用已有文档Tab。

#### Scenario: 两次打开不同章节
- **WHEN** 先打开FC15详情再打开异常详情
- **THEN** 同一文档Tab切换至对应章节，不产生重复Tab
