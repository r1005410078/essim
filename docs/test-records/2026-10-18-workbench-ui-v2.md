# 工作台 UI 整理与真实运行观测

## 范围

用户确认外围优化方案，表格继续遵循 NovaSheet。对应[提案](../../openspec/changes/unify-workbench-ui-and-observations/proposal.md)。这是 M07 的局部实施，不表示历史查看、离线回放或全部工作台验收完成。

- 合并模拟器／采集模型／设备项目树；保留稳定业务身份和目录操作。
- 保存配置、应用配置、提交运行值分别命名。运行状态与配置状态分开；连接上限放入运行设置。
- 属性分为点、Unit、端点；选择点只影响属性查看，不决定刷新或提交范围。
- 模型帮助按需展开，外围选择器使用 shadcn；底部问题读取真实模型校验结果。
- 应用预览列出保留、新增／恢复、移除、倍率变化及受影响点；明确停止并应用完整端点。
- 底部显示真实 RX／TX 和事件，提供方向筛选、暂停展示、字节详情及当前筛选视图 JSON 导出。移除表格上方原始 JSON 和未接入工具占位。

NovaSheet 的 theme、表头、行高、编辑器、选区、滚动、填充柄配置不变；RuntimeGrid 只新增选择点身份回调供属性栏查看。已访问表格仍保持挂载。

## 自动化验证

- `pnpm --filter @essim/workbench test`：14 文件、58 项通过。
- `pnpm --filter @essim/workbench build`：通过；Vite 仍提示既有大 bundle 警告，本次没有以拆包扩大改动。
- `cargo test -p essim-core -p essim-infrastructure -p libmodbus-rs --features essim-infrastructure/tcp-runtime,essim-infrastructure/vendored --quiet`：116 项通过。socket 测试在允许本地端口的环境运行。
- `python3 scripts/bundle_macos.py static`：macOS 调试包构建、ad-hoc 签名校验通过。
- 新测试核对观测时间与可靠记录文件一致、请求／响应字节与真实 socket 一致；覆盖前端去重、有界缓存、不同客户端发送确认、坏记录和运行重启序号复用。

## 原生与独立客户端验证

环境：本机 macOS 系统 WebView，独立 ESSim UI Check 测试包；1 个 TCP 端点、1 个 Unit、8 点（holding 10～13、coil 1000～1003，均为本次教学测试地址）；PyModbus 3.11.3，127.0.0.1:15324。测试配置由文件导入，无默认演示数据。

1. 验证统一新建菜单、CSV 导入、模型保存、默认 Unit／模型选择及完整端点应用预览。应用后保持停止。
2. 启动后独立 PyModbus FC15 写入4个 coil，FC1 回读均为 true；FC3 读取4个 holding 得到7。通信面板出现实际6条 RX／TX，完整响应标记为已写出 socket。
3. 遥控表4行均显示 true；点击第二行后属性显示 coil[1001]，分类切换保持原表格实例。
4. 暂停展示后请求数继续增加，面板仍保留原6条；继续后出现新增记录。TX 筛选显示4条，导出 JSON 内容核对为4条 TX，包含实际字节。
5. 模型点名清空产生真实校验问题，底部列出并可定位；恢复后问题清除，⌘S 保存。
6. 运行表 holding[10] 草稿改为42，提交修改计数1→0，独立客户端读取为[42,7,7,7]，证明新操作栏仍提交真实运行值。
7. 最终构建复核导入模态框内模板／列映射和模型属性的 shadcn 选择器，选项固定顺序；选择已有值不改变模型。
8. 停止后事件显示正在停止／资源已释放。窄窗口缩放检查操作不折行，点表使用自己的横向滚动。

实测图：[运行与报文](assets/ui-v2/runtime.png)、[模型编辑](assets/ui-v2/model.png)、[应用预览](assets/ui-v2/apply.png)、[校验问题](assets/ui-v2/problems.png)、[窄窗口](assets/ui-v2/compact.png)。概念图仍在 `docs/design/ui-workbench-v2`，不能当实测证据。

## 观测边界

每次查询后端最近100条可靠摘要，当前面板最多缓存500条；隐藏面板不轮询。暂停仅冻结展示，后端通信和可靠记录继续。暂停、高频通信或记录窗口滑动可能导致界面缺项；导出仅为当前筛选视图，不能当完整抓包。完整历史仍以运行文件为准，本次没有提供历史文件查看／回放。

响应准备不等于成功发送；仅同一客户端队列的 ResponseSent 将对应 TX 标记为已写出本机 socket，仍不代表远端应用或设备动作完成。准备字节已离开窗口时不伪造报文。

配置仍限本次会话；未接 SQLite。没有新增 RTU、主站运行或性能承诺。本次未重新完成 NovaSheet 中文输入、填充柄、大表压力及全部平台回归，相关前期证据适用范围不扩展。
