# 工程紧凑公共弹框

正式客户端的业务弹框统一使用 `@essim/ui/components/workbench-dialog`，样式位于 `packages/ui/src/workbench-dialog.css`。系统文件选择、保存路径窗口仍由操作系统提供；菜单、下拉列表、提示和历史原型不属于业务弹框。

## 外观与布局

- 亮色实底，5px 圆角，细边框和轻阴影；标题栏34px、输入框28px、操作按钮27px、操作栏最小42px。
- 默认宽472px；导入、迁移预览等可指定 `initialWidth`。默认高度随内容变化，上限为当前视口减16px。
- `DialogBody` 独立滚动，`DialogFooter` 固定在内容下方；缩放后按钮保持可见。字段默认88px标签列、10px列间距、7px行距。
- 关闭后重新打开恢复默认布局，不保存位置或大小。

## 交互

拖动标题栏或移动手柄改变位置；四边、四角和右下角手柄调整大小。所有操作限制在视口内。移动手柄的方向键每次移动8px；右下角手柄的方向键每次调整8px。Home、标题栏双击和复位按钮恢复自然尺寸与居中位置。

组件内部使用原生 `dialog.showModal()`，保留背景 inert、表单校验和顶层显示。初始焦点优先进入输入控件，Tab／Shift+Tab封住首尾，关闭后恢复焦点。Esc和关闭按钮走同一个 `onCancel`；`busy` 时禁止这两种关闭路径。业务方仍须按请求状态禁用取消／提交按钮，并保留原有异步任务清理。

下拉菜单应通过 portal 挂在最近的原生 dialog 上，以免被内容滚动区裁切或落到背景层。现有 `EnumSelect` 保持这一约定。

## 接入示例

```tsx
<WorkbenchDialog title="新建采集模型" onCancel={close} busy={busy}>
  <form onSubmit={submit}>
    <DialogBody>
      <label>名称<input autoFocus required value={name} onChange={event => setName(event.target.value)} /></label>
    </DialogBody>
    <DialogFooter>
      <button type="button" disabled={busy} onClick={close}>取消</button>
      <button type="submit" disabled={busy}>创建</button>
    </DialogFooter>
  </form>
</WorkbenchDialog>
```

组件挂载即打开，由父级条件渲染控制关闭；不要再调用 `showModal()`，不要给外层另加 padding、overflow 或固定最大高度。保留完整 form 包住内容和操作栏，以支持 Enter 和原生约束校验。操作栏最后一个按钮为主操作。

`/dialogs.html` 提供无后端交互夹具；`/dialogs.html?small=1` 提供360×420独立视口。夹具不进入产品入口，不代表协议能力。
