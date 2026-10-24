/** 在原生modal解除背景inert后恢复焦点；业务可按稳定身份解析新的触发节点。 */
export function closeDialogAndRestoreFocus(
  dialog: Pick<HTMLDialogElement, "close">,
  previous: Pick<HTMLElement, "isConnected" | "focus"> | null,
  restore?: () => void,
) {
  dialog.close();
  if (restore) restore();
  else if (previous?.isConnected) previous.focus();
}
