import type { FileTree } from "@pierre/trees";
import type { Target, projectNavigation } from "./model";

type Input = ReturnType<typeof projectNavigation>;
type Candidate = { target?: Target; path: string };
const sameTarget = (a: Target, b: Target) => a.kind === b.kind && a.id === b.id && a.unit === b.unit;
const canonical = (path: string) => path.replace(/\/$/, "");

/** 保存业务身份与祖先链，路径改变后仍可定位原节点，删除则逐级回退。 */
export function navigationFocusCandidates(input: Input, path: string): Candidate[] {
  const candidates: Candidate[] = [];
  let next = canonical(path);
  while (next) {
    candidates.push({ path: next, target: input.targets.get(next) ?? input.targets.get(`${next}/`) });
    next = next.includes("/") ? next.slice(0, next.lastIndexOf("/")) : "";
  }
  return candidates;
}

/** 从最新投影解析当前路径，不把相同名称的新对象当作旧对象。 */
export function navigationFocusPath(input: Input, candidates: Candidate[]): string | undefined {
  for (const candidate of candidates) {
    const path = candidate.target
      ? [...input.targets].find(([, target]) => sameTarget(target, candidate.target!))?.[0]
      : input.paths.find(path => canonical(path) === candidate.path);
    if (path) return path;
  }
}

/** 同时恢复DOM焦点和树的键盘游标；树组件负责虚拟行挂载与可见区域滚动。 */
export function restoreNavigationFocus(model: FileTree, root: Pick<HTMLElement, "focus"> & Partial<Pick<HTMLElement, "querySelectorAll">>, path: string | undefined) {
  root.focus();
  if (!path) { model.focusFirstItem(); return; }
  const segments = canonical(path).split("/");
  for (let i = 1; i < segments.length; i++) {
    const ancestor = model.getItem(segments.slice(0, i).join("/"));
    if (ancestor?.isDirectory() && "expand" in ancestor) ancestor.expand();
  }
  model.scrollToPath(path, { focus: true, offset: "nearest" });
  // 已挂载行直接拿回DOM焦点；虚拟行暂未挂载时，树根仍可处理键盘并按游标继续导航。
  const row = [...(root.querySelectorAll?.<HTMLElement>("[data-item-path]:not([data-item-parked])") ?? [])]
    .find(row => canonical(row.dataset.itemPath ?? "") === canonical(path));
  row?.focus();
}
