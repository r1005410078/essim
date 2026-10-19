# Modbus TCP 报文悬停解析

## 范围与实现

用户确认[效果图](../design/ui-workbench-v2/message-hover-decode.png)后实施，对应[提案](../../openspec/changes/add-modbus-message-hover-decode/proposal.md)。这是最近通信观测的展示功能，不改变 Rust/libmodbus 协议执行、运行值、可靠记录或 NovaSheet。

通信列表直接显示真实十六进制，列表和底部原始字节详情均支持悬停／键盘聚焦打开 shadcn 浮层，Escape关闭。字段按颜色分段，区分 MBAP 7字节与PDU；长内容可滚动，数值预览最多32项并注明总数。

覆盖FC01/02/03/04/05/06/15/16请求、正常响应和异常响应。校验MBAP协议／长度、PDU长度、数量、数据字节数、地址溢出、单coil写入编码及填充位。未知功能码保留原始字节并说明解析范围。

读响应仅借用当前最近窗口中同连接、事务号、Unit和功能码的唯一待处理请求地址；重复事务存在歧义时不关联。请求缺失则明确显示地址未知；位响应没有请求时不能确定实际点数与填充位，按数据位展示。寄存器仅解析原始u16，不推测设备的浮点类型、字序、倍率或工程值。

规范依据：[Modbus应用协议V1.1b3](https://www.modbus.org/file/secure/modbusprotocolspecification.pdf)第6、7节及[TCP Messaging Implementation Guide](https://www.modbus.org/file/secure/messagingimplementationguide.pdf)第3节。

## 验证

- 先增加失败测试，再实现纯展示解析与请求关联。最终 `pnpm --filter @essim/workbench test`：15文件、75项通过；本次新增17项，包含8功能码、异常、非法长度／数量／编码、位序、地址溢出、未知功能、长数据预览、连接隔离、重复及复用事务。
- `pnpm --filter @essim/workbench build`：通过；保留既有大bundle提示。
- `python3 scripts/bundle_macos.py static`：macOS调试包构建及ad-hoc签名校验通过。
- 本次仅改前端；未重复运行Rust测试，不将上轮116项结果算作本次新运行。

原生测试环境：macOS系统WebView，独立ESSim Hover Check包；1端点、1 Unit、8点，127.0.0.1:15325。PyModbus 3.11.3发送真实请求。地址为本次教学测试约定，不对应真实设备点表。

1. FC15写入coil[1000～1003]为true；浮层显示14字节、正确MBAP／PDU字段、4个true和末字节4个填充位。
2. FC1回读4个coil；关联对应请求后显示1000～1003，而非将填充位当作另外4点。
3. FC3读取holding[10～13]，浮层显示对应地址和原始值7。
4. FC3读取未定义地址0，服务端返回83 02；浮层显示原始功能码3和异常码2“非法数据地址”。
5. 使用Tab／Shift+Tab聚焦真实字节打开浮层，Escape关闭；列表及底部详情均验证，浮层跨Dockview边界显示完整。测试后停止端点，确认状态已停止。

实测截图：[FC15请求](assets/message-hover/fc15-request.png)、[FC1响应](assets/message-hover/fc1-response.png)、[异常响应](assets/message-hover/exception-response.png)。截图为实际运行界面，概念图不作为验收证据。

## 边界

仅解释完整TCP ADU的已记录字节，不提供TCP流重组、RTU CRC解析或所有Modbus功能。最近窗口仍有容量限制；关联不到请求时保持未知，不从当前模型倒推历史报文地址。响应写出socket不等于设备动作完成，解析浮层不改变原有发送确认状态。

原生检查覆盖实际报文、键盘打开／关闭、详情入口和停靠裁切；未新增Windows／Linux、触屏或所有小窗口尺寸验收。
