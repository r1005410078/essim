# 亮色滚动条调整

正式点表与普通滚动区共用 `packages/ui/src/scrollbar.css` 的颜色：默认滑块 `#dfe3e8`，hover `#a9b3c2`，按压 `#8795a8`。普通区域轨道透明；NovaSheet轨道 `#f7f8fa`、分隔线 `#edf0f4`，与工作台亮色背景接近。

NovaSheet配置表和运行表通过共用 `sheetTheme.scrollbar` 使用公开主题接口，保留原15px轨道和28px最小滑块长度，内缩改为4px，使可见滑块为7px。不修改第三方包、滚动状态或Canvas绘制。普通滚动条使用12px轨道、3px内缩。标准属性仅作为不支持WebKit伪元素时的回退，避免覆盖Chromium的自定义样式。

验证：

- `pnpm --filter @essim/workbench build` 通过，保留既有大chunk提示。
- 本机WebKit真实点表预览：默认滑块计算色 `rgb(223, 227, 232)`，轨道 `rgb(247, 248, 250)`；公开主题变量已生效。PageDown后scrollTop为335.5，横向键盘导航后scrollLeft为10，滚动仍可用。
- 公共弹框长内容预览：626px可视区域、1689px内容，12px滚动条；默认浅灰，指针落在滑块后视觉加深，内容和操作栏正常。
- 本轮为样式调整，未新增镜像样式断言；未重建Tauri应用或验证Windows／Linux及Firefox实际显示，按压色完成规则检查、未单独截图。

[点表默认截图](assets/scrollbars/idle.png) · [弹框默认](assets/scrollbars/dialog.png) · [弹框悬停](assets/scrollbars/dialog-hover.png)
