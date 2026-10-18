/** 在捕获阶段统一处理保存，避免表格或输入框吞掉快捷键。 */
export function bindSaveShortcut(target: EventTarget, save: () => void) {
  const onKeyDown = (event: Event) => {
    const key = event as KeyboardEvent;
    if (!(key.metaKey || key.ctrlKey) || key.altKey || key.shiftKey || key.key.toLowerCase() !== "s" || key.isComposing) return;
    key.preventDefault();
    key.stopPropagation();
    if (!key.repeat) save();
  };
  const options = { capture: true };
  target.addEventListener("keydown", onKeyDown, options);
  return () => target.removeEventListener("keydown", onKeyDown, options);
}

/** 隐藏的组件弹层不阻止保存；只有实际显示的对话框才占用快捷键。 */
export function hasOpenSaveDialog(root: Pick<Document, "querySelectorAll">): boolean {
  return [...root.querySelectorAll("dialog[open],[role=dialog],[role=alertdialog]")]
    .some(dialog => dialog.getClientRects().length > 0);
}
