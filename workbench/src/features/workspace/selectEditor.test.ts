import { afterEach, expect, it, vi } from "vitest";
import type { ReactElement } from "react";
import type { CellEditorOpenContext } from "@zhiguang/novasheet-core";
import { createSelectEditor } from "./selectEditor";

const mounted = vi.hoisted(() => ({ render: vi.fn(), unmount: vi.fn() }));
vi.mock("react-dom/client", () => ({ createRoot: () => mounted }));
afterEach(() => { vi.unstubAllGlobals(); vi.clearAllMocks(); });

function setup() {
  const host = { style: {}, addEventListener: vi.fn(), remove: vi.fn() };
  const createElement = vi.fn(() => host);
  vi.stubGlobal("document", { createElement });
  const ctx = {
    field: { id: "area", name: "数据区", type: "singleSelect", options: { choices: ["holding", "input"] } },
    value: "holding", rect: { x: 0, y: 0, width: 100, height: 30 },
    container: { appendChild: vi.fn() }, commit: vi.fn(), cancel: vi.fn(),
  } as unknown as CellEditorOpenContext;
  const editor = createSelectEditor();
  editor.open(ctx);
  const rendered = mounted.render.mock.calls.at(-1)![0] as ReactElement<{ ctx: CellEditorOpenContext }>;
  return { editor, ctx, callbacks: rendered.props.ctx, createElement, host };
}

it("挂载React编辑器，选项提交一次，关闭后的迟到取消不覆盖提交", async () => {
  const { ctx, callbacks, createElement, host } = setup();
  expect(createElement).toHaveBeenCalledExactlyOnceWith("div");
  callbacks.commit("input");
  callbacks.cancel();
  expect(ctx.commit).toHaveBeenCalledExactlyOnceWith("input");
  expect(ctx.cancel).not.toHaveBeenCalled();
  expect(host.remove).toHaveBeenCalledOnce();
  await Promise.resolve();
  expect(mounted.unmount).toHaveBeenCalledOnce();
});

it("取消及选择原值不产生修改", async () => {
  const first = setup();
  first.callbacks.cancel();
  await Promise.resolve();
  expect(first.ctx.cancel).toHaveBeenCalledOnce();
  expect(first.ctx.commit).not.toHaveBeenCalled();
  const second = setup();
  second.callbacks.commit("holding");
  expect(second.ctx.cancel).toHaveBeenCalledOnce();
  expect(second.ctx.commit).not.toHaveBeenCalled();
});

it("销毁后旧编辑器不能提交", () => {
  const { editor, ctx, callbacks } = setup();
  editor.destroy?.();
  callbacks.commit("input");
  expect(ctx.commit).not.toHaveBeenCalled();
  expect(ctx.cancel).not.toHaveBeenCalled();
});


it("菜单先关闭再派发选值时，选值优先提交，取消不能吞掉修改", async () => {
  const { ctx, callbacks } = setup();
  callbacks.cancel();
  callbacks.commit("input");
  await Promise.resolve();
  expect(ctx.commit).toHaveBeenCalledExactlyOnceWith("input");
  expect(ctx.cancel).not.toHaveBeenCalled();
});
