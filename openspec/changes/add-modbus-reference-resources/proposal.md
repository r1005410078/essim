## Why
用户要求报文解析改为点击打开，修复菱形箭头，并从“详细”进入项目内分类文档资源。

## What Changes
- 点击报文打开可交互解析浮层，使用正常三角箭头，支持关闭。
- 增加“详细”入口，复用Dockview文档Tab并定位对应章节。
- 新增与采集模型平级的文档资源树，按协议／Modbus分类；Markdown文件随客户端打包离线渲染。
- 编写中文Modbus详细指南，区分标准、教学例子和ESSim支持边界。

## Capabilities
### New Capabilities
- `protocol-reference-resources`: 分类Markdown参考资料与报文详情联动。
### Modified Capabilities
无。

## Impact
仅外围UI、前端资源与文档；不改NovaSheet、协议执行或配置数据库，不引入评级功能。
