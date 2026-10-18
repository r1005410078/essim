import { expect, it, vi } from "vitest";
import { bindSaveShortcut, hasOpenSaveDialog } from "./saveShortcut";

it("Mac与Windows保存键只保存一次并阻止浏览器保存，释放后不再处理", () => {
  const target = new EventTarget();
  const save = vi.fn();
  const cancel = bindSaveShortcut(target, save);
  const press = (modifiers: object) => {
    const event = Object.assign(new Event("keydown", { cancelable: true }), { key: "s", ...modifiers });
    target.dispatchEvent(event);
    return event.defaultPrevented;
  };
  expect(press({ metaKey: true })).toBe(true);
  expect(press({ ctrlKey: true, key: "S" })).toBe(true);
  expect(press({ ctrlKey: true, repeat: true })).toBe(true);
  expect(press({ ctrlKey: true, shiftKey: true })).toBe(false);
  expect(press({ ctrlKey: true, altKey: true })).toBe(false);
  expect(press({ ctrlKey: true, isComposing: true })).toBe(false);
  expect(save).toHaveBeenCalledTimes(2);
  cancel();
  expect(press({ metaKey: true })).toBe(false);
  expect(save).toHaveBeenCalledTimes(2);
});

it("组件隐藏dialog不会吞掉保存，真实显示的模态框仍阻止后台文档保存", () => {
  let visible = false;
  const root = { querySelectorAll: () => [{ getClientRects: () => visible ? [{}] : [] }] } as unknown as Pick<Document, "querySelectorAll">;
  expect(hasOpenSaveDialog(root)).toBe(false);
  visible = true;
  expect(hasOpenSaveDialog(root)).toBe(true);
});
