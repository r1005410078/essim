## ADDED Requirements

### Requirement: 工作台操作分层
工作台 MUST 清晰区分配置保存、配置应用与运行值提交，保留 Dockview 紧凑布局，移除未接入操作占位。

#### Scenario: 修改配置后
- **WHEN** 用户编辑端点配置
- **THEN** 显示未保存状态，保存后可应用，应用预览说明完整端点范围和应用后停止状态

### Requirement: 保留点表体验
外围整理 MUST 保留 NovaSheet 主题、表头、行高、编辑、选区、滚动与填充柄行为。

#### Scenario: 切换分类
- **WHEN** 用户在分类间切换
- **THEN** 既有表格实例保留，待提交修改和选区不因布局调整而丢失
