# 工程紧凑弹框统一与验证

## 实现范围

正式 `workbench/` 原有17处原生业务弹框均接入共享 `WorkbenchDialog`、`DialogBody`、`DialogFooter`：

| 位置 | 弹框 |
| --- | --- |
| App | 模型重命名／复制／删除、通用确认、未保存草稿、退出会话、创建模型／设备／模拟器 |
| 主站运行 | 通信设置、范围读取、RTU手动诊断 |
| 从站运行 | 范围复位、Unit复位确认、连接上限设置 |
| 导航 | 重命名、移动、复制、删除、新建文件夹 |
| 分组 | 按采集点分组创建与预览 |
| 应用 | 端点配置迁移预览与应用 |
| Unit点表 | 回写／另存模型、确认值捕获 |
| 导入 | 导入模型／追加点表 |

更换采集模型原先的 `window.confirm` 也改用统一确认组件，取消时保留现有覆盖与草稿，确认时按稳定端点／Unit身份修改当前草稿。原生 dialog 与 showModal 仅留在公共组件内。系统文件选择器保持系统样式，旧 `prototype/` 不做正式工程迁移。

## 自动检查

- 修改前基线：22个测试文件、115项通过。
- 修改后：24个测试文件、119项通过（新增视口边界／对边锚定缩放3项，以及原生语义／关闭保护结构1项）。
- `pnpm --filter @essim/ui typecheck` 通过。
- `pnpm --filter @essim/workbench build` 通过；Vite仍提示既有大体积chunk，不作为本轮拆包任务。
- macOS Tauri debug应用构建成功；独立验证包 `ESSim Dialog Check.app`，bundle ID `dev.essim.dialogcheck`，未重启用户原工作台会话。

## UI交互验证

使用本机WebKit浏览器夹具与macOS Tauri系统WebView，执行真实UI操作；浏览器夹具不连接后端。

- 初始焦点落在名称输入框；空名称提交触发原生必填提示；填写后Enter提交成功，关闭后焦点回到打开按钮。
- 指针拖动：472×426弹框从(404,147)移动至(204,67)。右下角缩放至669×536，底部操作栏仍42px高且可见。复位恢复居中及自然尺寸。
- 下拉选择TCP→RTU成功，无背景遮挡。
- 50行长内容：弹框限制为704px高（720px视口），内容滚动区626px、scrollHeight1689px，底部操作栏可见；收起长内容后恢复426px自然高度。
- 忙碌时关闭按钮禁用，Esc不关闭；解除忙碌后关闭按钮正常。
- 发现并修复WebKit末尾Tab短暂移出弹框的问题；修复后Tab从缩放手柄回到标题栏移动手柄，Shift+Tab反向闭环。方向键移动8px验证通过。
- 360×420窄视口：内容滚动，底部取消／创建按钮可见；无需横向移动整个弹框操作。
- 原生Tauri：实际新建模型、绑定模型创建模拟器成功；导入弹框及模板下拉正常；真实后端返回应用配置迁移预览；TCP连接上限设置弹框正常打开并取消。未启动协议通信。

截图：[创建模拟器](assets/compact-dialogs/native-create.png)、[导入追加](assets/compact-dialogs/native-import.png)、[应用预览](assets/compact-dialogs/native-apply.png)、[窄视口](assets/compact-dialogs/narrow.png)。

## 覆盖边界

所有业务入口已完成代码替换；交互验证覆盖共享行为和上述代表性真实流程，没有逐一重跑17类弹框的全部业务分支。Windows／Linux原生WebView、硬件串口、完整导入文件矩阵和协议互操作未在本轮重验；本轮不改变协议核心及保存／应用语义。
