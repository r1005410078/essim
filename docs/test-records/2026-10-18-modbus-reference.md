# 点击报文解析与内置 Modbus 文档资源

## 范围

用户要求修复解析箭头、点击打开解析、从“详细”进入Dockview Markdown文档，并确认“文档资源与采集模型平级”。对应[提案](../../openspec/changes/add-modbus-reference-resources/proposal.md)。

- 解析使用shadcn Popover，字节按钮点击／Enter打开；不再悬停自动打开。原生三角Arrow与可滚动内容分开，去除旋转方块产生的菱形。
- 浮层提供关闭与“详细”；根据功能码／异常定位文档章节，已有Tab更新定位参数，不产生重复Tab。
- 项目树新增“文档资源 / 协议 / Modbus / Modbus 详细指南”，与“采集模型”平级。内置资源独立于会话模型目录，不增加评级或配置数据库。
- 正文位于[资源文件](../../workbench/src/resources/protocols/modbus/guide.md)，分类维护见[资源说明](../../workbench/src/resources/README.md)。使用现有react-markdown／remark-gfm渲染，包含目录、表格和代码块，关闭原始HTML。
- 打开文档继续保留原观测端点；文档只读，不触发保存／应用，也不修改NovaSheet。

## 文档内容与边界

指南包含请求响应、四区与零基地址、MBAP／PDU、基础8功能码、异常、类型／字序／倍率、RTU与CRC、其他功能码、EMS扫描和联调排障。教学地址与数据注明约定，文档中介绍的标准功能不代表ESSim已支持。官方应用协议、TCP及串行指南附原始链接。

## 自动化验证

- `pnpm --filter @essim/workbench test`：17文件、82项通过，包含资源分类、稳定锚点、章节映射、Tab定位、代码块伪标题排除、12条TCP教学报文，以及真实Markdown渲染与资源缺失空态。
- `pnpm --filter @essim/workbench build`：通过，保留既有大bundle提示。
- `python3 scripts/bundle_macos.py static`：通过macOS调试包构建与ad-hoc签名校验。
- 本次没有修改Rust协议执行器，不重复引用旧测试作为本次执行结果。

## 原生检查

独立ESSim Reference Check测试包，macOS系统WebView。

- 已验证分类树、只读文档Tab、章节目录定位FC15、Markdown表格／代码块和资源信息栏；[文档截图](assets/modbus-reference/document.png)。

### 2026-10-19 补充实测

解锁后完成此前中断的检查。导入8点独立测试模型，以127.0.0.1:15326、Unit 1启动TCP从站；使用独立PyModbus客户端完成FC15写coil[1000～1003]、FC01读回四个true、FC03读取holding[10～13]四个7，以及未定义地址0返回异常02。

- 点击真实RX字节打开解析，显示FC15、零基范围1000～1003和四个true；箭头为三角形。[点击解析截图](assets/modbus-reference/click-parse.png)。
- Escape关闭并恢复触发按钮焦点，再次点击正常打开。
- 点击“详细”激活已有的“Modbus 详细指南”Tab，焦点定位FC15标题。[FC15跳转截图](assets/modbus-reference/detail-fc15.png)。
- 点击真实TX异常响应的“详细”，同一Tab定位“异常响应”标题；未增加重复文档Tab。[异常跳转截图](assets/modbus-reference/detail-exception.png)。
- 检查完成后停止独立测试端点，界面确认“已停止”、连接0/10、请求4、异常1。

此次资料不是规范译本，不扩展RTU、主站执行、完整历史、回放或规模验收范围。外部官方PDF链接与多平台行为不在本次原生验收范围内。
