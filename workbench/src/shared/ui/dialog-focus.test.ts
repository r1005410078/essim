import { expect, it, vi } from "vitest";
import { closeDialogAndRestoreFocus } from "@essim/ui/lib/dialog-focus";

it("先关闭原生modal再恢复调用方焦点，不能让背景inert拦截focus", () => {
  let open = true;
  const close = vi.fn(() => { open = false; });
  const restore = vi.fn(() => { expect(open).toBe(false); });
  const previous = { isConnected: true, focus: vi.fn() };
  closeDialogAndRestoreFocus({ close }, previous, restore);
  expect(close).toHaveBeenCalledOnce();
  expect(restore).toHaveBeenCalledOnce();
  expect(previous.focus).not.toHaveBeenCalled();
});

it("普通弹框仍返回触发控件，已移除的控件不再聚焦", () => {
  const previous = { isConnected: true, focus: vi.fn() };
  closeDialogAndRestoreFocus({ close() {} }, previous);
  expect(previous.focus).toHaveBeenCalledOnce();
  previous.isConnected = false;
  closeDialogAndRestoreFocus({ close() {} }, previous);
  expect(previous.focus).toHaveBeenCalledOnce();
});
