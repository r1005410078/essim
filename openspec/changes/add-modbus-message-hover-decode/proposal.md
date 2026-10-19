## Why
用户确认报文字节悬停解析效果图，需要从原始通信数据直接理解协议字段和读写范围。

## What Changes
- 通信列表和底部字节详情支持悬停／键盘聚焦解析，保留原始十六进制。
- 分色展示 MBAP、PDU、地址数量及值；覆盖 FC01/02/03/04/05/06/15/16 请求、响应与异常。
- 缺少上下文、未知功能码和畸形报文明示边界，不猜点类型、倍率或地址。

## Capabilities
### New Capabilities
- `modbus-message-hover`: 有界真实报文的展示解析。
### Modified Capabilities
无。

## Impact
仅前端展示解析与测试、文档。不改变 Rust 协议处理、可靠记录、NovaSheet 或通信操作。
