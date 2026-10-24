import { expect, it, vi } from "vitest";
import { FileTree } from "@pierre/trees";
import { navigationFocusCandidates, navigationFocusPath, restoreNavigationFocus } from "./navigationFocus";
import type { Target } from "./model";

const endpoint: Target = { kind: "endpoint", id: "e" };
const unit: Target = { ...endpoint, unit: "u" };
const folder: Target = { kind: "folder", id: "f" };
const input = (entries: [string, Target][]) => ({ paths: entries.map(([path]) => path), targets: new Map(entries) });
const original = input([["文件夹/", folder], ["文件夹/模拟器/", endpoint], ["文件夹/模拟器/Unit 1", unit]]);

it("取消返回原节点；重命名后按稳定ID定位新路径", () => {
  const candidates = navigationFocusCandidates(original, "文件夹/模拟器/Unit 1");
  expect(navigationFocusPath(original, candidates)).toBe("文件夹/模拟器/Unit 1");
  const renamed = input([["目录/", folder], ["目录/新模拟器/", endpoint], ["目录/新模拟器/Unit 1 · 新名称", unit]]);
  expect(navigationFocusPath(renamed, candidates)).toBe("目录/新模拟器/Unit 1 · 新名称");
});

it("删除Unit回所属模拟器，删除模拟器回原父目录；不借用同名新对象", () => {
  const candidates = navigationFocusCandidates(original, "文件夹/模拟器/Unit 1");
  expect(navigationFocusPath(input([["文件夹/", folder], ["文件夹/模拟器/", endpoint]]), candidates)).toBe("文件夹/模拟器/");
  expect(navigationFocusPath(input([["文件夹/", folder], ["文件夹/模拟器/", { ...endpoint, id: "other" }]]), candidates)).toBe("文件夹/");
  expect(navigationFocusPath(input([]), candidates)).toBeUndefined();
});

it("设备树中的Unit删除后返回原设备目录", () => {
  const tree = input([["设备/PCS/", { kind: "device", id: "d" }], ["设备/PCS/模拟器 · 单元", unit]]);
  const candidates = navigationFocusCandidates(tree, "设备/PCS/模拟器 · 单元");
  tree.targets.delete("设备/PCS/模拟器 · 单元");
  expect(navigationFocusPath(tree, candidates)).toBe("设备/PCS/");
});

it("恢复DOM焦点同时同步树键盘模型，展开祖先并滚动到虚拟列表外的目标", () => {
  const paths = ["模拟器/", ...Array.from({ length: 400 }, (_, i) => `模拟器/Unit ${i}`)];
  const model = new FileTree({ paths, initialExpansion: "closed" });
  const root = { focus: vi.fn() };
  restoreNavigationFocus(model, root, "模拟器/Unit 399");
  expect(root.focus).toHaveBeenCalledOnce();
  expect(model.getFocusedPath()).toBe("模拟器/Unit 399");
  expect(model.getVisibleCount()).toBe(401);
  expect(model.getSelectedPaths()).toEqual([]);
  model.cleanUp();
});

it("空树仍有可接收Tab和菜单键的树根焦点", () => {
  const model = new FileTree({ paths: [] });
  const root = { focus: vi.fn() };
  restoreNavigationFocus(model, root, undefined);
  expect(root.focus).toHaveBeenCalledOnce();
  expect(model.getFocusedPath()).toBeNull();
  model.cleanUp();
});

it("已重建的目标行获得DOM焦点，键盘游标不退回同名兄弟节点", () => {
  const model = new FileTree({ paths: ["模拟器/", "模拟器/新名称", "模拟器/兄弟"], initialExpansion: "open" });
  const target = { dataset: { itemPath: "模拟器/新名称" }, focus: vi.fn() };
  const sibling = { dataset: { itemPath: "模拟器/兄弟" }, focus: vi.fn() };
  const root = { focus: vi.fn(), querySelectorAll: () => [sibling, target] } as unknown as HTMLElement;
  restoreNavigationFocus(model, root, "模拟器/新名称");
  expect(target.focus).toHaveBeenCalledOnce();
  expect(sibling.focus).not.toHaveBeenCalled();
  expect(model.getFocusedPath()).toBe("模拟器/新名称");
  model.cleanUp();
});
