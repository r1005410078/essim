import {useNavigationDrag} from "./useNavigationDrag";
import {draggableTarget} from "./treeDrag";
import {folderSprite,folderSymbolId} from "./folderIcons";
import {TreeStatusLayer} from "./TreeStatusLayer";
import type {WorkspaceStore} from "./model";
import { useEffect, useRef } from "react";
import { FileTree, useFileTree, type FileTreeProps } from "@pierre/trees/react";
import { Layers, Server } from "lucide-react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@essim/ui/components/tooltip";
import type { Catalog } from "../../shared/api/contracts";
import type { NavigationSection, Target, projectNavigation } from "./model";
import spriteSheet from "./navigation-icons.svg?raw";
import { navigationFocusCandidates, navigationFocusPath, restoreNavigationFocus } from "./navigationFocus";

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
  [data-navigation-drop="true"] {box-shadow:inset 0 0 0 1px #83ace0;background:#edf3fc;}
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
    density: "default", itemHeight: 28, search: false, renaming: false, dragAndDrop: {canDrag:paths=>paths.every(path=>draggableTarget(current.current.input.targets.get(path))),canDrop:()=>false},
    composition: { contextMenu: { enabled: true, triggerMode: "right-click" } },
    icons: { set: "none", spriteSheet },
    unsafeCSS: treeCSS,
    renderRowDecoration: ({ item }) => {
      const path = String(item.path);
      const target = current.current.input.targets.get(path);
      let icon = "folder";
      if (target?.kind === "endpoint") {
        icon = target.unit ? "unit" : current.current.catalog?.endpoints.find(endpoint => endpoint.id === target.id)?.transport.kind === "rtu" ? "serial" : "endpoint";
      } else if (target?.kind === "model") icon = "model";
      else if (target?.kind === "device") icon = "device";
      else if (target?.kind === "resource") icon = "document";
      else if (target?.kind === "folder") return {icon:{name:folderSymbolId(target.id),width:16,height:16,viewBox:"0 0 24 24"}};
      else if (path.replace(/\/$/, "") === "采集模型") icon = "models";
      else if (path.replace(/\/$/, "") === "设备") icon = "device";
      else if (path.replace(/\/$/, "") === "文档资源") icon = "documents";
      return { icon: { name: `navigation-${icon}`, width: 16, height: 16, viewBox: "0 0 24 24" } };
    },
  });
  useNavigationDrag(model,input,catalog,store,section,open);
  const previousInput=useRef(input);
  const projected=useRef(false);
  useEffect(() => {
    const old=previousInput.current;
    const identity=(target:Target)=>`${target.kind}:${target.id}:${target.unit??''}`;
    const nextById=new Map([...input.targets].map(([path,target])=>[identity(target),path]));
    const remap=(path:string)=>{const target=old.targets.get(path);return target?nextById.get(identity(target)):input.paths.includes(path)?path:undefined;};
    const selection=model.getSelectedPaths().map(remap).filter((p):p is string=>!!p);
    const focus=model.getFocusedPath();
    const expanded=[...old.targets.keys()].filter(path=>{const item=model.getItem(path);return item&&'isExpanded' in item&&item.isExpanded();}).map(remap).filter((p):p is string=>!!p);
    // 固定分组同样保留展开状态；第一次投影沿用全部展开。
    for(const row of model.getVisibleRows(0,model.getVisibleCount()))if(row.isExpanded&&!old.targets.has(row.path))expanded.push(row.path);
    model.setIcons({set:"none",spriteSheet:spriteSheet.replace("</svg>",`${folderSprite(catalog?.navigation?.folders??[])}</svg>`)});
    model.resetPaths(input.paths,projected.current?{initialExpandedPaths:expanded}:undefined);
    projected.current=true;
    for(const path of model.getSelectedPaths())model.getItem(path)?.deselect();
    for(const path of selection)model.getItem(path)?.select();
    if(focus){const path=remap(focus);if(path)model.focusPath(path);}
    previousInput.current=input;
  }, [model, input, catalog?.navigation]);
  return <><FileTree model={model} style={{ flex: 1, minHeight: 0 }} {...props}
    renderContextMenu={props.renderContextMenu ? (item, context) => {
      const candidates = navigationFocusCandidates(current.current.input, String(item.path));
      const panel = model.getFileTreeContainer()?.closest<HTMLElement>(".navigation-panel");
      return props.renderContextMenu!(item, { ...context, restoreFocus: () => {
        // modal已关闭；同步更新模型再恢复焦点，不依赖WebView可被暂停的动画帧。
        if (document.querySelector("dialog[open]")) return;
        const host = model.getFileTreeContainer();
        const root = (host?.shadowRoot ?? host)?.querySelector<HTMLElement>('[role="tree"]');
        if (!root?.isConnected || !root.getClientRects().length) { if (panel?.isConnected) panel.focus(); return; }
        model.resetPaths(current.current.input.paths, {initialExpandedPaths:model.getVisibleRows(0,model.getVisibleCount()).filter(row=>row.isExpanded).map(row=>row.path)});
        restoreNavigationFocus(model, root, navigationFocusPath(current.current.input, candidates));
      } });
    } : undefined}
  /><TreeStatusLayer model={model} input={input} catalog={catalog} store={store} active={active}/></>;
}
