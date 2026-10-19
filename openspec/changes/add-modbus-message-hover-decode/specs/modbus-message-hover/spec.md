## ADDED Requirements

### Requirement: 字节悬停解析
通信列表及详情 MUST 通过悬停或键盘聚焦展示真实字节的 MBAP／PDU 字段及读写含义，保留 NovaSheet 原样。

#### Scenario: 四点写入
- **WHEN** RX 为 FC15 地址1000数量4且数据为0F
- **THEN** 显示 MBAP 7字节及 PDU，地址1000～1003依次为true，说明低位在前

### Requirement: 解析边界
解析 MUST 覆盖现有8功能码与异常响应，校验长度和字段，未知功能码保留原始数据。

#### Scenario: 非法长度
- **WHEN** MBAP声明长度与实际字节不符或数据被截断
- **THEN** 展示具体问题，不能越界读取或将其显示为有效值

### Requirement: 响应上下文
读响应 SHALL 仅在同连接、事务号、Unit及功能码唯一匹配请求时显示地址和准确位数量。

#### Scenario: 最近窗口缺少请求
- **WHEN** 只存在读响应
- **THEN** 展示原始值并注明地址／实际位数未知
