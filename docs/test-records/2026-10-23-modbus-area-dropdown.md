# 数据区下拉展示验证

日期：2026-10-23。

## 实际变更

- 模型与 Unit 配置共用 PointGrid：选项首行中文在前、英文标识在后，次行显示每地址位宽与读写属性；单元格及编辑触发器只显示中文。
- Markdown 分别说明四区用途、十进制功能码及访问限制，来源为 [Modbus Application Protocol V1.1b3 §4.3、§6](https://modbus.org/docs/Modbus_Application_Protocol_V1_1b3.pdf)。悬停与键盘聚焦均可查看；小于640px的视口向下展开，其他视口优先在右侧展示并由浮层处理边界。
- 底层数据保持英文标识，未修改协议模型、校验和文件交换格式。未知值原样显示，不伪装为合法数据区。

## 验证证据

- `pnpm --filter @essim/workbench test`：26个文件、124项通过，包含四区 Markdown 覆盖，以及中文格式化后复制仍为英文、粘贴与草稿提交保持英文、撤销和非法值展示。
- `pnpm --filter @essim/workbench build`：TypeScript和Vite构建通过；存在原有大块体积提示。
- 浏览器通过临时夹具挂载真实 PointGrid、DraftSource 和生产样式，未伪造协议后端。验证900×600桌面断点下的菜单和说明、方向键聚焦说明、Enter提交输入寄存器、鼠标提交线圈、Esc取消，以及319px宽度下说明完整可见。检查结束后移除临时夹具并停止预览。
- [实际下拉截图](2026-10-23-modbus-area-dropdown.png)。

## 边界

本次为前端展示修改，未运行真实协议互操作、Tauri系统WebView复核或大表性能测试。不据此完成M02总体验收。右侧属性栏及其他非点表选择器保持既有行为。
