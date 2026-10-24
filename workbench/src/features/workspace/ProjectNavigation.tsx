import {TreeStatusLayer} from "./TreeStatusLayer";
import type {WorkspaceStore} from "./model";
import { useEffect, useRef } from "react";
import { FileTree, useFileTree, type FileTreeProps } from "@pierre/trees/react";
import { Layers, Server } from "lucide-react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@essim/ui/components/tooltip";
import type { Catalog } from "../../shared/api/contracts";
import type { NavigationSection, Target, projectNavigation } from "./model";
import spriteSheet from "./navigation-icons.svg?raw";

export const navigationSections = ["simulators", "resources"] as const;
export const navigationLabels = { simulators: "模拟器", resources: "资源" };

export function NavigationTabs({ id, section, onChange }: {
  id: string;
  section: NavigationSection;
  onChange: (section: NavigationSection) => void;
}) {
  return <div className="navigation-tabs" role="tablist" aria-label="导航分类">
    {navigationSections.map((value, index) => <Tooltip key={value}>
      <TooltipTrigger asChild>
        <button type="button" role="tab" id={`${id}-${value}-tab`}
          className={`navigation-tab navigation-tab-${value}`}
          aria-label={navigationLabels[value]} aria-selected={section === value}
          aria-controls={`${id}-${value}-panel`} tabIndex={section === value ? 0 : -1}
          onClick={() => onChange(value)}
          onKeyDown={event => {
            if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
            event.preventDefault();
            const next = event.key === "Home" ? 0 : event.key === "End" ? 1 : 1 - index;
            const target = navigationSections[next]!;
            onChange(target);
            document.getElementById(`${id}-${target}-tab`)?.focus();
          }}>
          {value === "simulators" ? <Server size={16} /> : <Layers size={16} />}
        </button>
      </TooltipTrigger>
      <TooltipContent>{navigationLabels[value]}</TooltipContent>
    </Tooltip>)}
  </div>;
}

// 树组件在Shadow DOM内绘制。使用其装饰槽和sprite API补充业务图标，
// 仅调整槽位顺序，保留原有展开箭头、键盘操作和虚拟列表。
const treeCSS = `
  :host {
    color-scheme: light;
    --trees-bg-override: #fff;
    --trees-fg-override: #293347;
    --trees-fg-muted-override: #72777f;
    --trees-font-size-override: 13px;
    --trees-selected-bg-override: #edf3fc;
    --trees-selected-fg-override: #2865bb;
    --trees-selected-focused-border-color-override: transparent;
    --trees-accent-override: #2865bb;
    --trees-indent-guide-bg-override: transparent;
  }
  [data-item-section="spacing"] { order: 0; }
  [data-item-section="icon"] { order: 1; }
  [data-item-type="file"] > [data-item-section="icon"] { display: none; }
  [data-item-section="decoration"] { order: 2; flex: 0 0 16px; justify-content: center; }
  [data-item-section="content"] { order: 3; flex: 1 1 auto; min-width:0; }
  .navigation-status { order:5; display:flex; align-items:center; gap:4px; flex:0 0 auto; margin-left:auto; padding-right:6px; }
  .navigation-status:empty { display:none; }
  [data-item-section="action"] { order: 4; }
`;

export function NavigationTree({ input, catalog, section, open, store, active, ...props }: {
  store: WorkspaceStore;
  active: boolean;
  input: ReturnType<typeof projectNavigation>;
  catalog: Catalog | undefined;
  section: NavigationSection;
  open: (target: Target) => void;
} & Pick<FileTreeProps, "renderContextMenu">) {
  const current = useRef({ input, catalog, open });
  current.current = { input, catalog, open };
  const { model } = useFileTree({
    paths: [], initialExpansion: "open", flattenEmptyDirectories: false,
    sort: (left, right) => {
      if (section === "resources" && left.segments.length === 1 && right.segments.length === 1) {
        const order = ["采集模型", "设备", "文档资源"];
        return order.indexOf(left.basename) - order.indexOf(right.basename);
      }
      return Number(right.isDirectory) - Number(left.isDirectory) || left.basename.localeCompare(right.basename, "zh-CN", { numeric: true });
    },
    density: "default", itemHeight: 28, search: false, renaming: false, dragAndDrop: false,
    composition: { contextMenu: { enabled: true, triggerMode: "both" } },
    icons: { set: "none", spriteSheet },
    unsafeCSS: treeCSS,
    onSelectionChange: paths => {
      const target = current.current.input.targets.get(paths.at(-1) ?? "");
      if (target) current.current.open(target);
    },
    renderRowDecoration: ({ item }) => {
      const path = String(item.path);
      const target = current.current.input.targets.get(path);
      let icon = "folder";
      if (target?.kind === "endpoint") {
        icon = target.unit ? "unit" : current.current.catalog?.endpoints.find(endpoint => endpoint.id === target.id)?.transport.kind === "rtu" ? "serial" : "endpoint";
      } else if (target?.kind === "model") icon = "model";
      else if (target?.kind === "device") icon = "device";
      else if (target?.kind === "resource") icon = "document";
      else if (target?.kind === "folder") icon = "folder";
      else if (path.replace(/\/$/, "") === "采集模型") icon = "models";
      else if (path.replace(/\/$/, "") === "设备") icon = "device";
      else if (path.replace(/\/$/, "") === "文档资源") icon = "documents";
      return { icon: { name: `navigation-${icon}`, width: 16, height: 16, viewBox: "0 0 24 24" } };
    },
  });
  useEffect(() => { model.resetPaths(input.paths); }, [model, input]);
  return <><FileTree model={model} style={{ flex: 1, minHeight: 0 }} {...props} /><TreeStatusLayer model={model} input={input} catalog={catalog} store={store} active={active}/></>;
}
