import { afterEach, expect, it, vi } from "vitest";
import type { CellEditor } from "@zhiguang/novasheet-core";
import { createSelectEditor } from "./selectEditor";

class SelectNode extends EventTarget {
  value = "holding";
  style = {};
  setAttribute() {}
  appendChild() {}
  prepend() {}
  focus() {}
  remove() {}
}
afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});
it("WebKit先Enter/blur后change时仍提交新选项，迟到事件不重复提交", () => {
  vi.useFakeTimers();
  const select = new SelectNode();
  vi.stubGlobal("document", {
    createElement: (tag: string) => (tag === "select" ? select : {}),
  });
  const editor = createSelectEditor();
  const commit = vi.fn(() => editor.close?.());
  const cancel = vi.fn(() => editor.close?.());
  editor.open({
    field: {
      id: "area",
      name: "数据区",
      type: "singleSelect",
      options: { choices: ["holding", "input"] },
    },
    value: "holding",
    rect: { x: 0, y: 0, width: 100, height: 30 },
    container: { appendChild() {} },
    commit,
    cancel,
  } as unknown as Parameters<CellEditor["open"]>[0]);
  const enter = new Event("keydown");
  Object.defineProperty(enter, "key", { value: "Enter" });
  select.dispatchEvent(enter);
  select.dispatchEvent(new Event("blur"));
  expect(cancel).not.toHaveBeenCalled();
  select.value = "input";
  select.dispatchEvent(new Event("change"));
  vi.runAllTimers();
  expect(commit).toHaveBeenCalledExactlyOnceWith("input");
  expect(cancel).not.toHaveBeenCalled();
});
