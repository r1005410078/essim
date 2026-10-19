## Context
现有最近观测保留完整 TCP ADU，前端已有 shadcn Tooltip。用户确认的概念图为 docs/design/ui-workbench-v2/message-hover-decode.png。

## Goals / Non-Goals
**Goals:** 按字节可解释、错误不误导、悬停与键盘都可查看。
**Non-Goals:** 重写协议执行器；解码 RTU、设备自定义编码、工程值或全部 Modbus 功能。

## Decisions
- 纯 TypeScript 展示解析函数，按官方 V1.1b3 与 TCP Messaging Guide 核验长度和字段；没有网络副作用。
- 请求与响应根据真实 RX／TX 方向区分；只关联同连接、事务号、Unit、功能码的唯一待处理请求，重复或缺失时不补地址。
- 列表展示字节与摘要，shadcn Tooltip 在门户内显示以避免 Dockview 裁切；浮层限制宽高并支持滚动、焦点、Escape。
- 原始字节不变，位值仅按协议低位在前解释；读位响应缺少请求时不确定实际点数。

## Risks / Trade-offs
- 最近记录窗口缺失请求 → 显示无对应请求，不猜地址或填充位。
- 长数据占满屏幕 → 值预览有界并标注数量，完整原始内容仍可在详情查看。
- 不可靠／不合法输入 → 显示解析问题和实际字节，不显示虚构正常结果。

规范：[应用协议](https://www.modbus.org/file/secure/modbusprotocolspecification.pdf)第6、7节；[TCP指南](https://www.modbus.org/file/secure/messagingimplementationguide.pdf)第3节。
