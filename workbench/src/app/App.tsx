import { WorkbenchDialog, DialogBody, DialogFooter } from "@essim/ui/components/workbench-dialog";
import { WorkspaceIllustration } from "../features/workspace/WorkspaceIllustration";
import { initialWorkbenchLayout } from "../features/workspace/initialLayout";
import { UnitSelector } from "../features/workspace/UnitSelector";
import { ConfigurationStatus } from "../features/workspace/RuntimeToolbar";
import { ToolbarIcon } from "../features/workspace/ToolbarIcon";
import {UnitPointEditor,CaptureInitialsDialog} from "../features/workspace/UnitPointEditor";
import {ModelPicker} from "../features/workspace/ModelPicker";
import {overrideCount,emptyOverrides} from "../features/workspace/unitOverrides";
import { PropertyPanel, PropertyGroup, PropertyRow as Property, PropertyActions, PropertyIconButton, PropertyHint, PropertyScope } from "@essim/ui/components/property-panel";
import {ResourcePanel} from "../features/workspace/ResourcePanel";
import { createPortal } from "react-dom";
import {BatchPanel} from "../features/workspace/BatchPanel";
import { PartitionDialog } from "../features/workspace/PartitionDialog";
import { SerialPortPicker } from "../features/workspace/SerialPortPicker";
import {ResourceDocument} from "../features/resources/ResourceDocument";
import {resources} from "../features/resources/catalog";
import {Button} from "@essim/ui/components/button";
import {ActionMenu} from "../features/workspace/ActionMenu";
import {ObservationPanel} from "../features/workspace/ObservationPanel";
import { RuntimeGrid } from "../features/workspace/RuntimeGrid";
import { MasterRuntimeControls } from "../features/workspace/MasterRuntimeControls";
import type {MasterPoint} from "../shared/api/contracts";
import { SlaveRuntimeControls } from "../features/workspace/SlaveRuntimeControls";
import type { RawValue } from "../shared/api/contracts";
import { NavigationDialog, type NavigationRequest } from "../features/workspace/NavigationDialog";
import { EnumSelect } from "../features/workspace/EnumSelect";
import { bindSaveShortcut, hasOpenSaveDialog } from "../features/workspace/saveShortcut";
import { nextStation, applicationStateLabel, isEndpointApplied } from "../features/workspace/model";
import { ApplyModelDialog } from "../features/workspace/ApplyModelDialog";
import {
  createContext,
  useContext,
  useId,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import {
  DockviewReact,
  themeLight,
  type DockviewApi,
  type DockviewReadyEvent,
  type IDockviewPanelProps,
  type IDockviewPanelHeaderProps,
  type IDockviewHeaderActionsProps,
} from "dockview-react";
import { NavigationTabs, NavigationTree, navigationLabels, navigationSections } from "../features/workspace/ProjectNavigation";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@essim/ui/components/tooltip";
import {
  Plus,
  SlidersHorizontal,
  Activity,
  Save,
  Play,
  Check,
  MoreHorizontal,
  Table2,
  Server,
  ChevronLeft,
  ChevronRight,
  FolderOpen,
  X,
  CircleCheck,
  Info,
  FileCheck2,
  ListStart,
  FileSpreadsheet,
  BookOpen,
} from "lucide-react";
import "dockview-react/dist/styles/dockview.css";
import { createWorkspaceClient } from "../shared/api/workspace";
import type {
  Category,
  CollectionModel,
  EndpointConfig,
  UnitConfig,
} from "../shared/api/contracts";
import {
  WorkspaceStore,
  errorMessage,
  projectNavigation,
  documentPosition,
  type Target,
  type NavigationSection,
} from "../features/workspace/model";
import {
  SheetViews,
  resolveUnitSheet,
  categories,
} from "../features/workspace/sheets";

import { PointEditor, ModelDetails } from "../features/workspace/PointEditor";
import { ImportPoints } from "../features/workspace/ImportPoints";
import { ModelDraft } from "../features/workspace/pointDraft";
import {
  armCloseGuard,
  armEditCommands,
  armSaveCommand,
  closeWorkbenchWindow,
} from "../shared/api/window";

type DialogKind = "model" | "endpoint" | "device" | null;
interface WorkbenchContext {
  store: WorkspaceStore;
  openTool:(tool:"batch"|"resources")=>void;
  sheets: SheetViews;
  open: (target: Target) => void;
  selected: Target | null;
  dialog: (kind: DialogKind) => void;
  closeDocument: (id: string, close: () => void) => void;
  removedModel: (id: string) => void;
  removedEndpoint: (id: string) => void;
}
const workbenchTheme = { ...themeLight, gap: 6 };
const Context = createContext<WorkbenchContext | null>(null);
function useWorkbench() {
  const context = useContext(Context);
  if (!context) throw new Error("缺少工作台上下文");
  useSyncExternalStore(context.store.subscribe, context.store.snapshot);
  return context;
}
const attempt = (promise: Promise<unknown>) => {
  void promise.catch(() => {});
};
function Icon({
  label,
  disabled,
  children,
  onClick,
  saveDocument,
}: {
  saveDocument?: string;
  label: string;
  disabled?: boolean;
  children: ReactNode;
  onClick?: () => void;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span className="icon-wrap">
          <button
            type="button"
            data-save-document={saveDocument}
            className="icon"
            aria-label={label}
            disabled={disabled}
            onClick={onClick}
          >
            {children}
          </button>
        </span>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}
function Empty({ children }: { children: ReactNode }) {
  return <div className="empty">{children}</div>;
}
function ProjectPanel() {
  const { store, open, dialog, removedModel, removedEndpoint, openTool } = useWorkbench();
  const navigationId = useId();
  const [section, setSection] = useState<NavigationSection>("simulators");
  const originalCatalog = store.view?.catalog;
  const draftResourceKey = JSON.stringify(
    [...store.modelDrafts.values()].map((d) => [d.id, d.name, d.persisted]),
  );
  const catalog = useMemo(
    () =>
      originalCatalog
        ? {
            ...originalCatalog,
            models: [
              ...originalCatalog.models.map((model) => ({
                ...model,
                name: store.modelDrafts.get(model.id)?.name ?? model.name,
              })),
              ...[...store.modelDrafts.values()]
                .filter((d) => !d.persisted)
                .map((d) => ({
                  id: d.id,
                  name: d.name,
                  points: [],
                })),
            ],
          }
        : undefined,
    [originalCatalog, draftResourceKey],
  );
  const [partitioning,setPartitioning]=useState(false);
  const [navigationAction,setNavigationAction]=useState<NavigationRequest|null>(null);
  const [importing, setImporting] = useState(false);
  const [appendImport, setAppendImport] = useState<string | null>(null);
  const [resourceAction, setResourceAction] = useState<{
    kind: "rename" | "copy" | "delete";
    id: string;
  } | null>(null);
  const [rootMenu, setRootMenu] = useState<{ x: number; y: number } | null>(
    null,
  );
  const inputs = useMemo(
    () => ({
      simulators: projectNavigation(catalog, "simulators"),
      resources: projectNavigation(catalog, "resources"),
    }),
    [catalog],
  );
  const newItems = section === "simulators" ? [
    { label: "模拟器…", disabled: !store.view || store.busy, action: () => dialog("endpoint") },
    { label: "模拟器文件夹…", disabled: !store.view || store.busy, action: () => setNavigationAction({ kind: "newFolder" }) },
    { label: "按采集点分组创建…", disabled: !store.view?.catalog.models.length || store.busy, action: () => setPartitioning(true) },
  ] : [
    { label: "采集模型…", disabled: !store.view || store.busy, action: () => dialog("model") },
    { label: "设备…", disabled: !store.view || store.busy, action: () => dialog("device") },
    { label: "导入新模型…", disabled: !store.view || store.busy, action: () => setImporting(true) },
  ];
  return (
    <div
      className="panel column navigation-panel"
      tabIndex={0}
      aria-label="项目导航面板"
      onContextMenu={(event) => {
        if (!event.defaultPrevented) {
          event.preventDefault();
          setRootMenu({ x: event.clientX, y: event.clientY });
        }
      }}
      onKeyDown={(event) => {
        if (
          !event.defaultPrevented &&
          event.target === event.currentTarget &&
          event.shiftKey &&
          event.key === "F10"
        ) {
          event.preventDefault();
          setRootMenu({ x: 24, y: 120 });
        }
      }}
    >
      <div className="navigation-heading">
        <NavigationTabs id={navigationId} section={section} onChange={value => { setSection(value); setRootMenu(null); }} />
        <div className="navigation-actions">
          <ActionMenu label={`新建${navigationLabels[section]}`} items={newItems}><Plus size={14}/></ActionMenu>
          <ActionMenu label="更多导航操作" items={[
            { label: "批量任务", action: () => openTool("batch") },
            { label: "运行资源", action: () => openTool("resources") },
          ]}><MoreHorizontal size={14}/></ActionMenu>
        </div>
      </div>
      {navigationSections.map(treeSection => {
        const input = inputs[treeSection];
        return <div key={treeSection} role="tabpanel" className="navigation-tree-panel"
          id={`${navigationId}-${treeSection}-panel`} aria-labelledby={`${navigationId}-${treeSection}-tab`}
          hidden={section !== treeSection}>
      {input.paths.length ? (
        <NavigationTree input={input} catalog={catalog} section={treeSection} open={open}
          renderContextMenu={(item, context) => {
            const target = input.targets.get(String(item.path));
            const section=treeSection === "resources" && String(item.path).split("/")[0] === "文档资源"?"documents":treeSection;
            const act = (action: () => void) => {
              context.close({ restoreFocus: false });
              action();
            };
            // 菜单脱离Dockview的裁切／层叠上下文，保留树组件识别门户所需的标记。
            return createPortal(
              <div
                role="menu"
                className="tree-context-menu"
                style={{
                  left: Math.min(
                    context.anchorRect.left,
                    window.innerWidth - 210,
                  ),
                  top: Math.min(
                    context.anchorRect.bottom,
                    window.innerHeight - 330,
                  ),
                }}
                data-file-tree-context-menu-root="true"
              >
                {section==="documents"&&<button role="menuitem" disabled={target?.kind!=="resource"} onClick={()=>target&&act(()=>open(target))}>打开文档</button>}
                {section==="simulators" && <>
                  <button role="menuitem" onClick={()=>act(()=>dialog("endpoint"))}>新建模拟器…</button>
                  <button role="menuitem" onClick={()=>act(()=>setNavigationAction({kind:"newFolder",target:target?.kind==="folder"?target:undefined}))}>新建文件夹…</button>
                  {(target?.kind==="endpoint" || target?.kind==="folder") && <>
                    {target.kind==="endpoint" && <button role="menuitem" onClick={()=>act(()=>setNavigationAction({kind:"copy",target}))}>复制模拟器…</button>}
                    <button role="menuitem" onClick={()=>act(()=>setNavigationAction({kind:"rename",target}))}>重命名…</button>
                    <button role="menuitem" onClick={()=>act(()=>setNavigationAction({kind:"move",target}))}>移动到…</button>
                    <button role="menuitem" onClick={()=>act(()=>setNavigationAction({kind:"delete",target}))}>删除…</button>
                  </>}
                </>}
                {section==="resources" && <button
                  role="menuitem"
                  onClick={() => act(() => dialog("model"))}
                >
                  新建采集模型
                </button>}
                {section === "resources" && <button role="menuitem" onClick={() => act(() => setImporting(true))}>导入新模型…</button>}
                {target?.kind === "model" && (
                  <>
                    <button
                      role="menuitem"
                      onClick={() => act(() => open(target))}
                    >
                      打开
                    </button>
                    <button
                      role="menuitem"
                      onClick={() =>
                        act(() => {
                          open(target);
                          setResourceAction({ kind: "rename", id: target.id });
                        })
                      }
                    >
                      重命名…
                    </button>
                    <button
                      role="menuitem"
                      onClick={() =>
                        act(() =>
                          setResourceAction({ kind: "copy", id: target.id }),
                        )
                      }
                    >
                      复制模型…
                    </button>
                    <button
                      role="menuitem"
                      onClick={() =>
                        act(() => {
                          open(target);
                          setAppendImport(target.id);
                        })
                      }
                    >
                      导入追加…
                    </button>
                    <button
                      role="menuitem"
                      onClick={() =>
                        act(() => {
                          open(target);
                          setResourceAction({ kind: "delete", id: target.id });
                        })
                      }
                    >
                      删除…
                    </button>
                    {["json", "csv", "xlsx"].map((format) => (
                      <button
                        role="menuitem"
                        key={format}
                        onClick={() =>
                          act(() => {
                            const saved = store.view?.catalog.models
                              .filter((m) => m.id === target.id)
                              .at(-1);
                            if (saved)
                              attempt(
                                store.client.exportPointFile?.(
                                  saved.id,
                                  format,
                                ) ?? Promise.reject(Error("文件服务不可用")),
                              );
                          })
                        }
                      >
                        导出 {format.toUpperCase()}…
                      </button>
                    ))}
                  </>
                )}
              </div>,
              document.body,
            );
          }}
        />
      ) : (
        <div className="workspace-side-empty">
          <WorkspaceIllustration kind="cabinet"/>
          <h2>{store.view ? "添加第一台模拟器" : "等待连接核心"}</h2>
          <p>{store.view ? "点击上方 ＋，配置端点和单元。" : "连接 Rust 核心后，开始配置设备。"}</p>
        </div>
      )}
        </div>;
      })}
      {partitioning&&<PartitionDialog store={store} close={()=>setPartitioning(false)}/>}
      {navigationAction && <NavigationDialog store={store} request={navigationAction} close={()=>setNavigationAction(null)} open={open} removed={removedEndpoint} />}
      <div className="navigation-note">会话配置 · 退出后不保留</div>
      {rootMenu && createPortal(
        <div
          className="tree-menu-backdrop"
          onPointerDown={() => setRootMenu(null)}
        >
          <div
            role="menu"
            className="tree-context-menu root-tree-menu"
            style={{ left: rootMenu.x, top: rootMenu.y }}
            onPointerDown={(e) => e.stopPropagation()}
          >
            {newItems.map(item => <button key={item.label} role="menuitem" disabled={item.disabled}
              onClick={() => { setRootMenu(null); item.action(); }}>{item.label}</button>)}
          </div>
        </div>,
        document.body,
      )}
      {importing && (
        <ImportPoints
          store={store}
          close={() => setImporting(false)}
          created={(id) => open({ kind: "model", id })}
        />
      )}
      {appendImport && (
        <ImportPoints
          store={store}
          target={store.modelDraft(appendImport)}
          close={() => setAppendImport(null)}
        />
      )}
      {resourceAction && (
        <ResourceActionDialog
          removed={removedModel}
          store={store}
          action={resourceAction}
          close={() => setResourceAction(null)}
          open={(id) => open({ kind: "model", id })}
          locate={(id, unit) => open({ kind: "endpoint", id, unit })}
        />
      )}
    </div>
  );
}
function Welcome() {
  const { store, dialog } = useWorkbench();
  return (
    <div className="workspace-welcome"><div className="workspace-welcome-content">
      <WorkspaceIllustration kind="testbench"/>
      <h2>Modbus 工作台</h2>
      <p>从一张采集模型开始，连接 EMS 与储能设备。</p>
      <div className="welcome-actions">
        <button disabled={!store.view || store.busy} onClick={() => dialog("model")}>新建采集模型</button>
        <button disabled={!store.view || store.busy} onClick={() => dialog("endpoint")}>新建模拟器</button>
      </div>
      <p className="workspace-welcome-note">配置 TCP／RTU 端点，应用后运行，在下方观察真实通信。</p>
    </div></div>
  );
}
function ResourceActionDialog({
  removed,
  store,
  action,
  close,
  open,
  locate,
}: {
  store: WorkspaceStore;
  removed: (id: string) => void;
  action: { kind: "rename" | "copy" | "delete"; id: string };
  close: () => void;
  open: (id: string) => void;
  locate: (id: string, unit: string) => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const draft = store.modelDraft(action.id);
  const latest = store.view?.catalog.models.find((m) => m.id === action.id);
  const [name, setName] = useState(
    action.kind === "copy"
      ? `${latest?.name ?? draft?.name ?? "模型"} 副本`
      : (draft?.name ?? ""),
  );
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const references = [
    ...(store.view?.catalog.endpoints ?? []),
    ...(store.view?.catalog.applied ?? []),
  ].flatMap((e) =>
    e.units.filter((u) => u.model.id === action.id).map((u) => ({ e, u })),
  );
  const submit = async () => {
    setBusy(true);
    setError("");
    try {
      if (action.kind === "rename") {
        draft?.change((s) => {
          s.name = name;
        });
        open(action.id);
      } else if (action.kind === "copy") {
        if (!latest) throw Error("请先保存模型再复制。");
        const id = crypto.randomUUID();
        await store.execute({
          kind: "copyModel",
          source: { id: action.id },
          id,
          name,

        });
        open(id);
      } else {
        if (latest)
          await store.execute({
            kind: "deleteModel",
            id: action.id,

          });
        store.modelDrafts.delete(action.id);
        removed(action.id);
        store.changed();
      }
      close();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <WorkbenchDialog ref={ref} onCancel={close} title={<>
        {action.kind === "rename"
          ? "重命名模型草稿"
          : action.kind === "copy"
            ? "复制模型"
            : "删除模型"}
      </>} busy={busy}>
      <DialogBody>
      {action.kind === "delete" ? (
        <>
          <p>确认删除“{draft?.name}”及其未保存草稿？</p>
          {references.length > 0 && (
            <>
              <p className="error">模型仍被引用，须先更换或解除引用。</p>
              {references.map(({ e, u }, i) => (
                <button
                  key={i}
                  onClick={() => {
                    close();
                    locate(e.id, u.id);
                  }}
                >
                  {" "}
                  {e.name} → Unit {u.station}
                </button>
              ))}
            </>
          )}
        </>
      ) : (
        <label>
          名称
          <input
            aria-label="模型操作名称"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
        </label>
      )}
      {error && (
        <p role="alert" className="error">
          {error}
        </p>
      )}
      </DialogBody><DialogFooter>
        <button onClick={close} disabled={busy}>
          取消
        </button>
        <button
          disabled={
            busy ||
            (action.kind !== "delete" && !name.trim()) ||
            (action.kind === "delete" && references.length > 0)
          }
          onClick={() => void submit()}
        >
          确认
        </button>
      </DialogFooter>
    </WorkbenchDialog>
  );
}
function currentUnit(
  store: WorkspaceStore,
  endpoint: EndpointConfig | undefined,
) {
  return (
    endpoint?.units.find(
      (unit) => unit.id === store.selectedUnits.get(endpoint.id),
    ) ?? endpoint?.units[0]
  );
}
function EndpointPanel({ params, api }: { params: { id: string }; api:IDockviewPanelProps["api"] }) {
  const [visible,setVisible]=useState(api.isVisible);
  useEffect(()=>{const listener=api.onDidVisibilityChange(event=>setVisible(event.isVisible));return()=>listener.dispose();},[api]);
  const { store, sheets } = useWorkbench();
  const endpoint = store.endpoint(params.id);
  const runtimeReaders=useRef(new Map<string,()=>string[]>());
  const [masterPoints,setMasterPoints]=useState<Record<string,Record<string,MasterPoint>>>({});
  const updateMasterPoints=(id:string,points:Record<string,MasterPoint>)=>{setMasterPoints(old=>({...old,[id]:points}));updateLiveValues(id,Object.fromEntries(Object.entries(points).flatMap(([key,p])=>p.value?[[key,p.value]]:[])));};
  const [liveValues,setLiveValues]=useState<Record<string,Record<string,RawValue>>>({});
  const updateLiveValues=(id:string,values:Record<string,RawValue>|undefined)=>setLiveValues(previous=>{
    if(JSON.stringify(previous[id])===JSON.stringify(values))return previous;
    if(!values){const next={...previous};delete next[id];return next;}
    return {...previous,[id]:values};
  });
  const mode=store.endpointModes.get(params.id)??"configuration";
  const [capture,setCapture]=useState<string[]|null>(null);
  const selectionReaders=useRef(new Map<string,()=>string[]>());
  const [captureError,setCaptureError]=useState("");
  const [applying, setApplying] = useState(false);
  const [resetting,setResetting]=useState(false);
  const [visited, setVisited] = useState<string[]>([]);
  const unit = currentUnit(store, endpoint);
  const key = unit?.id ?? "";
  const category = (store.categories.get(key) ?? "measurement") as Category;
  // 已访问单元保持挂载，NovaSheet自身保存选区与滚动；不在切换时重建表格。
  useEffect(() => {
    if (key)
      setVisited((previous) =>
        previous.includes(key) ? previous : [...previous, key],
      );
  }, [key]);
  if (!endpoint) return <Empty>端点不存在。</Empty>;
  const saved = store.view?.catalog.endpoints.find(
    (item) => item.id === endpoint.id,
  );
  const applied = store.view?.catalog.applied.find(
    (item) => item.id === endpoint.id,
  );
  const model =
    unit &&
    store.view?.catalog.models.find(
      (model) =>
        model.id === unit.model.id,
    );
  const choose = (next: Category) => {
    store.categories.set(key, next);
    store.changed();
  };
  const selector=<UnitSelector units={endpoint.units} value={unit?.id??""} onChange={id=>{store.flushUnitEditors.get(key)?.();store.selectUnit(endpoint.id,id);}}/>;
  const application=<><ToolbarIcon label="应用配置" disabled={store.busy||!saved||store.endpointDirty(endpoint.id)||isEndpointApplied(store.view?.catalog,saved,store.view?.applicationMatches)} onClick={()=>setApplying(true)}><FileCheck2 size={16}/></ToolbarIcon>
    <ConfigurationStatus dirty={store.endpointDirty(endpoint.id)} applied={isEndpointApplied(store.view?.catalog,endpoint,store.view?.applicationMatches)} hasSnapshot={!!applied} master={endpoint.role==="master"} modelDirty={!!(unit&&store.modelDrafts.get(unit.model.id)?.dirty)}/></>;
  const viewTabs=<div className="unit-view-tabs" role="tablist" aria-label="单元视图">{([['configuration','配置',SlidersHorizontal],['runtime','运行',Activity]] as const).map(([value,label,Icon])=><ToolbarIcon key={value} label={`${label}视图`} role="tab" aria-selected={mode===value} onClick={()=>{store.flushUnitEditors.get(key)?.();store.endpointModes.set(endpoint.id,value);store.changed();}}><Icon size={15}/></ToolbarIcon>)}</div>;
  const address=<span className="point-toolbar-address" title={endpoint.transport.kind==="tcp"?`${endpoint.transport.address}:${endpoint.transport.port}`:endpoint.transport.path}>{endpoint.transport.kind==="tcp"?`${endpoint.transport.address}:${endpoint.transport.port}`:endpoint.transport.path}</span>;
  const saveEndpoint={label:"保存端点配置",disabled:store.busy||!store.endpointDirty(endpoint.id),action:()=>attempt(store.saveEndpointDraft(endpoint.id))};
  const captureAction = <ToolbarIcon label="将所选确认值设为默认初始值" disabled={!unit} onClick={() => {
    if (!unit) return;
    try {
      setCaptureError("");
      const ids = selectionReaders.current.get(`${key}:${category}`)?.() ?? [];
      if (!ids.length) throw Error("请在运行表中选择需要捕获的点");
      const actual = store.view?.catalog.values[unit.id]?.model;
      if (!actual) throw Error("请先应用配置");
      const pending = endpoint.role === "master" ? sheets.pendingMaster(unit.id, actual) : sheets.pending(unit.id, actual);
      if (pending.some(p => ids.includes(p.point.id))) throw Error("所选点含未提交运行草稿，请先处理");
      setCapture(ids);
    } catch (e) {
      setCaptureError(errorMessage(e));
    }
  }}><ListStart size={15}/></ToolbarIcon>;
  const runtimeToolbar={
    start:<>{selector}<i className="toolbar-divider"/><ToolbarIcon label="保存端点配置" data-save-document={`endpoint:${endpoint.id}`} disabled={saveEndpoint.disabled} onClick={saveEndpoint.action}><Save size={16}/></ToolbarIcon>{application}</>,
    capture:captureAction,
    model:<span className="point-toolbar-model">{model?.name??"未绑定模型"}</span>,
    end:<>{address}<i className="toolbar-divider"/>{viewTabs}</>,
  };
  return (
    <div className="panel column">
      {(mode==="configuration"&&!unit)&&<div className="toolbar document-actions compact-point-toolbar">{selector}<i className="toolbar-divider"/>
        <ToolbarIcon label="保存端点配置" data-save-document={`endpoint:${endpoint.id}`} disabled={saveEndpoint.disabled} onClick={saveEndpoint.action}><Save size={16}/></ToolbarIcon>{application}
        <span className="point-toolbar-model">{model?.name??"未绑定模型"}</span><span className="spacer"/>{address}{viewTabs}
      </div>}
      <div style={{display:mode==="runtime"?"contents":"none"}}>
      <SlaveRuntimeControls toolbar={runtimeToolbar} visible={visible&&mode==="runtime"} store={store} sheets={sheets} endpoint={endpoint} unit={unit}
        model={unit?(store.view?.catalog.values[unit.id]?.model??model):model} category={category}
        readVisiblePoints={()=>runtimeReaders.current.get(`${key}:${category}`)?.()??[]} onValues={updateLiveValues}/>
      <MasterRuntimeControls toolbar={runtimeToolbar} visible={visible&&mode==="runtime"} store={store} sheets={sheets} endpoint={endpoint} unit={unit}
        model={unit?(store.view?.catalog.values[unit.id]?.model??model):model}
        readVisiblePoints={()=>runtimeReaders.current.get(`${key}:${category}`)?.()??[]} onPoints={updateMasterPoints}/>
      </div>

      {captureError&&<div className="error panel-note" role="alert">{captureError}</div>}
      {capture&&unit&&<CaptureInitialsDialog store={store} endpoint={endpoint.id} unit={unit.id} points={capture} close={()=>setCapture(null)}/>}
      {resetting && unit && <ConfirmAction title={`恢复 ${unit.name} 初值`} message={endpoint.role === "slave" ? "仅恢复选定Unit已应用模型的初值，不应用未保存或待应用修改。" : "仅清空选定Unit本地缓存并标记过期，不向远端写入。"} close={()=>setResetting(false)} submit={()=>store.execute({kind:"resetUnit",id:endpoint.id,unit:unit.id})} />}
      {applying && (
        <ApplyModelDialog
          store={store}
          endpoint={saved ?? endpoint}
          pendingWrites={endpoint.role==="master"?endpoint.units.reduce((sum,u)=>sum+sheets.pendingMaster(u.id,store.view?.catalog.values[u.id]?.model??{id:"",name:"",points:[]}).length,0):0}
          discardWrites={()=>{for(const u of endpoint.units)sheets.discardMaster(u.id);setMasterPoints({});setLiveValues({});}}
          close={() => setApplying(false)}
        />
      )}
      {mode==="configuration" && unit && store.modelDrafts.get(unit.model.id)?.dirty && (
        <div className="panel-note">引用模型有未保存修改，请先保存模型；当前显示已保存点表。</div>
      )}
      {mode==="configuration"&&unit&&<div className="sheet-area unit-configuration"><UnitPointEditor store={store} endpoint={endpoint.id} unit={unit.id} toolbar={{start:<>{selector}<i className="toolbar-divider"/></>,afterSave:application,context:address,end:<><i className="toolbar-divider"/>{viewTabs}</>,menu:[saveEndpoint]}}/></div>}
      <div className="sheet-area" style={{display:mode==="runtime"?undefined:"none"}}>
        {!unit ? (
          <Empty>在右侧属性栏添加 Unit，并引用一个已保存的采集模型。</Empty>
        ) : (
          <>
            {visited.map((id) => {
              const viewUnit = endpoint.units.find((item) => item.id === id);
              if (!viewUnit) return null;
              const { model: resolvedModel, values: resolvedValues } = resolveUnitSheet(
                store.view?.catalog, endpoint, viewUnit, store.endpointDirty(endpoint.id) ? {[endpoint.id]:store.draftMatches.get(endpoint.id) ?? store.view?.applicationMatches?.[endpoint.id] ?? {}} : store.view?.applicationMatches,
              );
              const appliedValues=store.view?.catalog.values[id];
              const hasLive=liveValues[id]!==undefined;
              const viewModel=appliedValues?.model??resolvedModel;
              const viewValues=hasLive?{model:viewModel??null,raw:liveValues[id]!,stale:false}:appliedValues??resolvedValues;
              return categories.map(([value, label]) => (
                <div
                  key={`${id}:${value}`}
                  className="sheet-slot"
                  style={{
                    display:
                      id === key && value === category ? "block" : "none",
                  }}
                >
                  <RuntimeGrid
                    onSelectionReader={read=>{const identity=`${id}:${value}`;if(read)selectionReaders.current.set(identity,read);else selectionReaders.current.delete(identity);}}
                    onPoint={point=>{if(id!==key||value!==category)return;if(point)store.runtimePoint.set(id,point);else store.runtimePoint.delete(id);store.changed();}}
                    onReader={read=>{const identity=`${id}:${value}`;if(read)runtimeReaders.current.set(identity,read);else runtimeReaders.current.delete(identity);}}
                    data={sheets.get(
                      id,
                      value,
                      viewModel,
                      viewValues,
                      endpoint.role,
                      masterPoints[id],
                    )}
                  />
                  {!viewModel?.points.some(
                    (point) => point.category === value,
                  ) && <div className="sheet-empty">暂无{label}点</div>}
                </div>
              ));
            })}
          </>
        )}
      </div>
      <div className="category-tabs" role="tablist" aria-label="四遥分类" style={{display:mode==="runtime"?undefined:"none"}}>
        <button
          aria-label="上一分类"
          onClick={() =>
            choose(
              categories[
                (categories.findIndex(([value]) => value === category) + 3) % 4
              ]![0],
            )
          }
        >
          <ChevronLeft size={13} />
        </button>
        <button
          aria-label="下一分类"
          onClick={() =>
            choose(
              categories[
                (categories.findIndex(([value]) => value === category) + 1) % 4
              ]![0],
            )
          }
        >
          <ChevronRight size={13} />
        </button>
        {categories.map(([value, label]) => (
          <button
            role="tab"
            aria-selected={category === value}
            className={category === value ? "active" : ""}
            key={value}
            onClick={() => choose(value)}
          >
            {label}
          </button>
        ))}
        <span className="spacer" />
        <span className="muted">NovaSheet · 草稿不触发通信</span>
      </div>
    </div>
  );
}
function ModelPanel({ params }: { params: { id: string } }) {
  const { store } = useWorkbench();
  return <PointEditor store={store} id={params.id} />;
}
function ConfirmAction({title,message,close,submit}:{title:string;message:string;close:()=>void;submit:()=>Promise<unknown>}) {
 const ref=useRef<HTMLDialogElement>(null);const [error,setError]=useState("");const [busy,setBusy]=useState(false);
 return <WorkbenchDialog ref={ref} onCancel={e=>{e.preventDefault();e.stopPropagation();if(!busy)close();}} title={<>{title}</>} busy={busy}><DialogBody><p>{message}</p>{error && <p role="alert" className="error">{error}</p>}</DialogBody><DialogFooter><button disabled={busy} onClick={close}>取消</button><button disabled={busy} onClick={async()=>{setBusy(true);try{await submit();close();}catch(e){setError(errorMessage(e));}finally{setBusy(false);}}}>确认</button></DialogFooter></WorkbenchDialog>;
}
function CloseDraftDialog({
  store,
  request,
  cancel,
}: {
  store: WorkspaceStore;
  request: { id: string; close: () => void };
  cancel: () => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const endpointId = request.id.startsWith("endpoint:") ? request.id.slice(9) : null;
  const modelId = request.id.startsWith("model:") ? request.id.slice(6) : request.id;
  const finish = () => {
    request.close();
    cancel();
  };
  return (
    <WorkbenchDialog ref={ref} onCancel={e=>{e.preventDefault();e.stopPropagation();if(!busy)cancel();}} title={<>{endpointId ? "模拟器" : "模型"}有未保存修改</>} busy={busy}>
      <DialogBody>
      <p>保存仅保留本次会话。请选择如何关闭。</p>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      </DialogBody><DialogFooter>
        <button disabled={busy} onClick={cancel}>
          取消
        </button>
        <button
          disabled={busy}
          onClick={() => {
            if(endpointId) store.discardEndpoint(endpointId);
            const d = endpointId ? undefined : store.modelDrafts.get(modelId);
            if (d && !d.persisted) store.modelDrafts.delete(modelId);
            else d?.discard();
            store.changed();
            finish();
          }}
        >
          放弃修改
        </button>
        <button
          disabled={busy}
          onClick={async () => {
            setBusy(true);
            try {
              if(endpointId) {
                const endpoint=store.endpoint(endpointId); if(!endpoint) throw Error("模拟器不存在");
                await store.saveEndpointDraft(endpointId);
              } else await store.saveModelDraft(modelId);
              if (endpointId ? store.endpointDirty(endpointId) : store.modelDrafts.get(modelId)?.dirty) {
                setError("保存期间又有新修改，请再次保存或取消。");
                return;
              }
              finish();
            } catch (e) {
              setError(errorMessage(e));
            } finally {
              setBusy(false);
            }
          }}
        >
          保存并关闭
        </button>
      </DialogFooter>
    </WorkbenchDialog>
  );
}
function ExitDialog({ cancel }: { cancel: () => void }) {
  const ref = useRef<HTMLDialogElement>(null);
  return (
    <WorkbenchDialog ref={ref} onCancel={cancel} title={<>退出本次会话？</>}>
      <DialogBody>
      <p>当前所有配置（包括已保存配置）、文件夹结构及未保存草稿，退出后都会丢失。</p>
      </DialogBody><DialogFooter>
        <button onClick={cancel}>取消退出</button>
        <button onClick={() => void closeWorkbenchWindow()}>放弃并退出</button>
      </DialogFooter>
    </WorkbenchDialog>
  );
}

function InspectorPanel() {
  const { store, selected, open } = useWorkbench();
  const [search, setSearch] = useState("");
  const [scope,setScope]=useState("unit");
  const [modelChange,setModelChange]=useState<{endpoint:string;unit:string;model:string}|null>(null);
  const inspector=useRef<HTMLElement>(null);
  const endpoint =
    selected?.kind === "endpoint" ? store.endpoint(selected.id) : undefined;
  const unit = currentUnit(store, endpoint);
  const catalog = store.view?.catalog;
  useEffect(()=>setModelChange(null),[endpoint?.id,unit?.id]);
  const changeModel=(endpointId:string,unitId:string,model:string)=>{
    const current=store.endpoint(endpointId);
    if(!current || !current.units.some(item=>item.id===unitId))return;
    store.unitDrafts.delete(unitId);
    store.edit({...current,units:current.units.map(item=>item.id===unitId?{...item,model:{id:model},overrides:emptyOverrides()}:item)});
  };
  useEffect(()=>{
    const error=store.lastError;if(!error || error.endpointId!==endpoint?.id)return;
    setSearch("");
    requestAnimationFrame(()=>{
      const target=inspector.current?.querySelector<HTMLElement>(`[data-config-field="${CSS.escape(error.unitId && error.field === "name" ? "unit.name" : error.field ?? "")}"]`);
      const details=target?.closest("details");if(details)details.open=true;
      target?.querySelector<HTMLElement>("input,button")?.focus({preventScroll:true});
      target?.scrollIntoView({block:"nearest"});
    });
  },[store.lastError,endpoint?.id]);
  const device =
    selected?.kind === "device"
      ? catalog?.devices.find((device) => device.id === selected.id)
      : undefined;
  const edit = (next: EndpointConfig) => store.edit(next);
  const patch = (value: Partial<UnitConfig>) => {
    if (endpoint && unit)
      edit({
        ...endpoint,
        units: endpoint.units.map((item) =>
          item.id === unit.id ? { ...item, ...value } : item,
        ),
      });
  };
  const selectedPoint=unit?(catalog?.values[unit.id]?.model??catalog?.models.find(model=>model.id===unit.model.id))?.points.find(point=>point.id===store.runtimePoint.get(unit.id)):undefined;
  const latest = catalog?.models ?? [];
  const applied = catalog?.applied
    .find((item) => item.id === endpoint?.id)
    ?.units.find((item) => item.id === unit?.id);
  const add = () => {
    const model = latest[0];
    if (!model || !endpoint) return;
    const station=nextStation(endpoint);
    if(station===undefined)return;
    const id = crypto.randomUUID();
    edit({
      ...endpoint,
      units: [
        ...endpoint.units,
        {
          id,
          name: `单元 ${station}`,
          station,
          enabled: true,
          model: { id: model.id },
          device: null,
        },
      ],
    });
    store.selectUnit(endpoint.id, id);
  };
  if(selected?.kind==="resource"){
    const resource=resources.find(resource=>resource.id===selected.id);
    return <PropertyPanel title="文档资源" icon={<BookOpen size={14}/>} subject={resource?.title??"文档不存在"}><PropertyGroup title="资源信息"><Property label="分类">{resource?.folders.join(" / ")}</Property><Property label="格式">Markdown</Property><Property label="访问">内置 · 只读</Property></PropertyGroup></PropertyPanel>;
  }
  if (selected?.kind === "model")
    return <ModelDetails store={store} id={selected.id} open={open} />;
  const canAdd =
    !!endpoint &&
    latest.length > 0 &&
    endpoint.units.length < 247 &&
    !(
      endpoint.transport.kind === "rtu" &&
      endpoint.role === "slave" &&
      endpoint.units.length > 0
    );
  return (
    <><PropertyPanel ref={inspector} subject={endpoint
      ? `${endpoint.transport.kind.toUpperCase()} ${endpoint.role === "slave" ? "从站" : "主站"}${unit ? ` / Unit ${unit.station}` : ""}`
      : (device?.name ?? "未选择对象")}
      status={endpoint && store.endpointDirty(endpoint.id) ? "未保存" : undefined}
      search={{ value: search, onChange: setSearch }}
      filters={endpoint && <PropertyScope value={scope} onChange={setScope} options={[{value:"point",label:"点"},{value:"unit",label:"Unit"},{value:"endpoint",label:"端点"}]}/>}>
        {!endpoint ? (
          device ? (
            <PropertyGroup title="设备">
              <Property label="名称" field="unit.name">{device.name}</Property>
              <Property label="类型">{device.kind}</Property>
            </PropertyGroup>
          ) : (
            <div className="workspace-side-empty workspace-side-empty--inspector">
              <WorkspaceIllustration kind="inspector"/>
              <h2>查看设备属性</h2>
              <p>在左侧选择模拟器或设备，<br/>查看端点、单元和点表属性。</p>
            </div>
          )
        ) : (
          <>
            {scope==="point"&&!search&&<PropertyGroup title="当前点">{selectedPoint?<><Property label="点名">{selectedPoint.name}</Property><Property label="数据区">{selectedPoint.area}</Property><Property label="零基地址">{selectedPoint.address}</Property><Property label="类型">{selectedPoint.valueType}</Property><Property label="倍率">{selectedPoint.scale}</Property><Property label="单位">{selectedPoint.unit||"—"}</Property><Property label="允许外部写入">{selectedPoint.slaveWritable?"是":"否"}</Property><PropertyHint>选择仅用于查看属性，刷新与提交范围独立。</PropertyHint></>:<Empty>在运行表选择一个点查看属性。</Empty>}</PropertyGroup>}
            <PropertyGroup title="单元配置" hidden={!search&&scope!=="unit"}>
              <Property label="当前单元" field="units">
                <PropertyActions>
                  <UnitSelector units={endpoint.units} value={unit?.id??""} onChange={id=>{if(unit)store.flushUnitEditors.get(unit.id)?.();store.selectUnit(endpoint.id,id);}}/>
                  <PropertyIconButton
                    label={
                      canAdd
                        ? "添加 Unit"
                        : "请先创建采集模型；RTU从站仅允许一个单元"
                    }
                    disabled={!canAdd || store.busy}
                    onClick={add}
                  >
                    <Plus size={19} />
                  </PropertyIconButton>
                </PropertyActions>
              </Property>
              {unit && (
                <>
                  <Property label="Unit ID" field="station">
                    <input
                      aria-label="线上站号"
                      type="number"
                      min="1"
                      max="247"
                      value={unit.station}
                      onChange={(event) =>
                        patch({ station: Number(event.target.value) })
                      }
                    />
                  </Property>
                  <Property label="名称" field="name">
                    <input
                      aria-label="单元名称"
                      value={unit.name}
                      onChange={(event) => patch({ name: event.target.value })}
                    />
                  </Property>
                  <Property label="Unit备注"><input aria-label="Unit备注" value={unit.note ?? ""} onChange={e=>patch({note:e.target.value})} /></Property>
                  <Property label="删除单元"><button onClick={()=>edit({...endpoint,units:endpoint.units.filter(u=>u.id!==unit.id)})}>从草稿移除 {unit.name}</button></Property>
                  <Property label="启用">
                    <input
                      type="checkbox"
                      aria-label="启用单元"
                      checked={unit.enabled}
                      onChange={(event) =>
                        patch({ enabled: event.target.checked })
                      }
                    />
                  </Property>
                  <Property label="关联设备" field="device">
                    <PropertyActions>
                      <EnumSelect label="关联设备" value={unit.device ?? ""} onChange={v=>patch({device:v || null})} options={[["","独立模拟器"],...(catalog?.devices ?? []).map(d=>[d.id,d.name] as const)]} />
                      <PropertyIconButton
                        label="定位关联设备"
                        disabled={!unit.device}
                        onClick={() =>
                          unit.device &&
                          open({ kind: "device", id: unit.device })
                        }
                      >
                        <FolderOpen size={17} />
                      </PropertyIconButton>
                      <PropertyIconButton
                        label="解除设备关联"
                        disabled={!unit.device}
                        onClick={() => patch({ device: null })}
                      >
                        <X size={16} />
                      </PropertyIconButton>
                    </PropertyActions>
                  </Property>
                  <Property label="采集模型" field="model">
                    <PropertyActions>
                      <EnumSelect label="采集模型" value={unit.model.id} onChange={v=>{if(overrideCount(unit.overrides)||store.unitDrafts.get(unit.id)?.dirty)setModelChange({endpoint:endpoint.id,unit:unit.id,model:v});else changeModel(endpoint.id,unit.id,v);}} options={latest.map(m=>[m.id,m.name] as const)} />
                      <PropertyIconButton
                        label="打开采集模型"
                        onClick={() =>
                          open({ kind: "model", id: unit.model.id })
                        }
                      >
                        <FolderOpen size={17} />
                      </PropertyIconButton>
                    </PropertyActions>
                  </Property>
                </>
              )}
            </PropertyGroup>
            <PropertyGroup title="端点配置" hidden={!search&&scope!=="endpoint"}>
              <Property label="模拟器备注"><input aria-label="模拟器备注" value={endpoint.note ?? ""} onChange={e=>edit({...endpoint,note:e.target.value})} /></Property>
              <Property label="角色">
                {endpoint.role === "slave" ? "从站" : "主站"}
              </Property>
              <Property label="协议">
                Modbus {endpoint.transport.kind.toUpperCase()}
              </Property>
              <Property label="名称" field="name">
                <input
                  aria-label="模拟器名称"
                  value={endpoint.name}
                  onChange={(event) =>
                    edit({ ...endpoint, name: event.target.value })
                  }
                />
              </Property>
              {endpoint.transport.kind === "tcp" ? (
                <>
                  <Property
                    label={endpoint.role === "slave" ? "监听地址" : "目标地址"}
                  >
                    <input
                      aria-label={endpoint.role === "slave" ? "监听地址" : "目标地址"}
                      value={endpoint.transport.address}
                      onChange={(event) => {
                        if (endpoint.transport.kind === "tcp")
                          edit({
                            ...endpoint,
                            transport: {
                              ...endpoint.transport,
                              address: event.target.value,
                            },
                          });
                      }}
                    />
                  </Property>
                  <Property label="端口" field="transport">
                    <input
                      type="number"
                      aria-label="端口"
                      min="1"
                      max="65535"
                      value={endpoint.transport.port}
                      onChange={(event) => {
                        if (endpoint.transport.kind === "tcp")
                          edit({
                            ...endpoint,
                            transport: {
                              ...endpoint.transport,
                              port: Number(event.target.value),
                            },
                          });
                      }}
                    />
                  </Property>
                </>
              ) : (
                <>
                  <Property label="串口路径" field="transport">
                    <SerialPortPicker client={store.client} value={endpoint.transport.path} onChange={path => {
                      if(endpoint.transport.kind === "rtu") edit({...endpoint, transport:{...endpoint.transport,path}});
                    }}/>
                  </Property>
                  <Property label="波特率" field="transport"><EnumSelect label="波特率" value={String(endpoint.transport.baud)} onChange={v=>{if(endpoint.transport.kind==="rtu")edit({...endpoint,transport:{...endpoint.transport,baud:Number(v)}});}} options={[9600,19200,38400,57600,115200].map(v=>[String(v),String(v)])} /></Property>
                  <Property label="校验位" field="transport"><EnumSelect label="校验位" value={endpoint.transport.parity} onChange={v=>{if(endpoint.transport.kind==="rtu")edit({...endpoint,transport:{...endpoint.transport,parity:v as "none"|"even"|"odd"}});}} options={[["none","无"],["even","偶校验"],["odd","奇校验"]]} /></Property>
                  <Property label="停止位" field="transport"><EnumSelect label="停止位" value={String(endpoint.transport.stopBits)} onChange={v=>{if(endpoint.transport.kind==="rtu")edit({...endpoint,transport:{...endpoint.transport,stopBits:Number(v) as 1|2}});}} options={[["1","1"],["2","2"]]} /></Property>
                  <Property label="数据位">8</Property>
                </>
              )}
              <Property label="状态">
                <span className="run-status">
                  <span className="status-dot" />
                  {applicationStateLabel(store.view?.applicationStates?.[endpoint.id])}
                </span>
              </Property>
            </PropertyGroup>

          </>
        )}
      {endpoint && <PropertyHint>切换单元仅切换视图，不影响运行与草稿。</PropertyHint>}
    </PropertyPanel>
    {modelChange && <ConfirmAction title="更换采集模型" message="更换模型将丢弃当前 Unit 覆盖和点表草稿，是否继续？" close={()=>setModelChange(null)} submit={async()=>changeModel(modelChange.endpoint,modelChange.unit,modelChange.model)}/>}</>
  );
}
function ProblemsPanel(){
 const {store,open}=useWorkbench();
 const issues=[...store.modelDrafts.values()].flatMap(draft=>(draft.preview?.errors??[]).map(issue=>({draft,issue})));
 return <div className="panel column observation-panel"><div className="observation-tools"><strong>问题</strong><span className="muted">{issues.length+(store.error?1:0)} 项</span><span className="spacer"/><span className="muted">来自实际校验结果</span></div><div className="problem-list">{store.error&&<div className="panel-note error">{store.error}</div>}{issues.map(({draft,issue},index)=><button className="problem-row" key={`${draft.id}:${index}`} onClick={()=>{open({kind:"model",id:draft.id});requestAnimationFrame(()=>store.focusModelIssue.get(draft.id)?.(issue));}}><span className="error">●</span><strong>{draft.name}</strong><span>{issue.message}</span></button>)}{!issues.length&&!store.error&&<div className="observation-empty"><WorkspaceIllustration kind="inspector"/><span>当前没有校验问题</span></div>}</div></div>;
}
function Observations({params,api}:IDockviewPanelProps<{view?:string}>){
 const {store,selected,open}=useWorkbench();
 return <ObservationPanel onReference={anchor=>open({kind:"resource",id:"modbus-guide",anchor})} store={store} endpointId={selected?.kind==="endpoint"?selected.id:undefined} kind={params?.view??"events"} api={api}/>;
}
function CreateDialog({
  kind,
  close,
}: {
  kind: Exclude<DialogKind, null>;
  close: () => void;
}) {
  const { store, open } = useWorkbench();
  const ref = useRef<HTMLDialogElement>(null);
  const [name, setName] = useState("");
  const [transport, setTransport] = useState("tcp");
  const [role, setRole] = useState<"slave" | "master">("slave");
  const [address, setAddress] = useState("127.0.0.1");
  const [port, setPort] = useState("1502");
  const [path, setPath] = useState("");
  const [baud, setBaud] = useState("19200");
  const [deviceKind, setDeviceKind] = useState("PCS");
  const models=store.view?.catalog.models ?? [];
  const [modelId,setModelId]=useState(models.length===1 ? models[0].id : "");
  const [unitName,setUnitName]=useState("Unit 1");
  const [station,setStation]=useState("1");
  const [newModelName,setNewModelName]=useState("");
  useEffect(()=>{if(!modelId && models.length===1) setModelId(models[0].id);},[models,modelId]);
  useEffect(() => {
    store.error="";store.lastError=null;store.changed();

  }, []);
  const title =
    kind === "model"
      ? "新建采集模型"
      : kind === "device"
        ? "新建设备"
        : "新建模拟器";
  async function submit(event: React.FormEvent) {
    event.preventDefault();
    const id = crypto.randomUUID();
    try {
      if (kind === "model") {
        const model: CollectionModel = { id, name, points: [] };
        await store.execute({
          kind: "saveModel",
          model,

        });
        open({ kind: "model", id });
      } else if (kind === "device") {
        await store.execute({
          kind: "createDevice",
          device: { id, name, kind: deviceKind },
        });
        open({ kind: "device", id });
      } else {
        const endpoint: EndpointConfig = {
          id,
          name,
          role,
          transport:
            transport === "tcp"
              ? { kind: "tcp", address, port: Number(port) }
              : {
                  kind: "rtu",
                  path,
                  baud: Number(baud),
                  parity: "none",
                  dataBits: 8,
                  stopBits: 1,
                },
          units: [{id:crypto.randomUUID(),name:unitName,station:Number(station),enabled:true,model:{id:modelId==="__new__"?crypto.randomUUID():modelId},device:null}],
        };
        await store.execute(modelId==="__new__"?{kind:"createEndpointWithModel",endpoint,model:{id:endpoint.units[0]!.model.id,name:newModelName.trim(),points:[]}}:{kind:"createEndpoint",endpoint});
        store.endpointModes.set(id,"configuration");
        open({ kind: "endpoint", id });
      }
      close();
    } catch {
      /* 错误由真实应用响应展示，保留草稿供修正。 */
    }
  }
  return (
    <WorkbenchDialog ref={ref} onCancel={e=>{e.preventDefault();e.stopPropagation();if(!store.busy)close();}} aria-label={title} title={<>{title}</>} busy={store.busy}>
      <form onSubmit={submit}>
        <DialogBody>
        <label>
          名称
          <input
            autoFocus
            required
            value={name}
            onChange={(event) => setName(event.target.value)}
          />
        </label>
        {kind === "device" && (
          <label>
            设备类型
            <input
              required
              value={deviceKind}
              onChange={(event) => setDeviceKind(event.target.value)}
            />
          </label>
        )}
        {kind === "endpoint" && (
          <>
            <label>首个Unit名称<input value={unitName} onChange={e=>setUnitName(e.target.value)} required /></label>
            <label>站号<input type="number" min="1" max="247" value={station} onChange={e=>setStation(e.target.value)} required /></label>
            <label>采集模型<ModelPicker models={models} value={modelId} newName={newModelName} onChange={(id,name)=>{setModelId(id);setNewModelName(name);}}/></label>
            <label>
              通信角色
              <EnumSelect label="通信角色" value={role} onChange={v=>setRole(v as "master"|"slave")} options={[["slave","从站"],["master","主站"]]} />
            </label>
            <label>
              传输
              <EnumSelect label="传输" value={transport} onChange={setTransport} options={[["tcp","TCP"],["rtu","RTU 串口"]]} />
            </label>
            {transport === "tcp" ? (
              <>
                <label>
                  IPv4 地址
                  <input
                    required
                    value={address}
                    onChange={(event) => setAddress(event.target.value)}
                  />
                </label>
                <label>
                  端口
                  <input
                    required
                    type="number"
                    min="1"
                    max="65535"
                    value={port}
                    onChange={(event) => setPort(event.target.value)}
                  />
                </label>
              </>
            ) : (
              <>
                <div>
                  <span>串口路径</span>
                  <SerialPortPicker client={store.client} value={path} onChange={setPath} required/>
                </div>
                <label>
                  波特率
                  <EnumSelect label="波特率" value={baud} onChange={setBaud} options={[9600,19200,38400,57600,115200].map(v=>[String(v),String(v)])} />
                </label>
                <p className="muted">
                  首版创建参数：无校验、8 数据位、1 停止位。
                </p>
              </>
            )}
          </>
        )}
        <p className="muted">保存到本次会话，不启动通信；退出后不保留。</p>
        {store.error && (
          <p role="alert" className="error">
            {store.error}{store.lastError?.relatedUnitId && `（冲突Unit：${store.lastError.relatedUnitId}）`}
          </p>
        )}
        </DialogBody><DialogFooter>
          <button type="button" disabled={store.busy} onClick={close}>
            取消
          </button>
          <button className="primary" disabled={store.busy || !name.trim() || (kind === "endpoint" && (modelId==="__new__"?!newModelName.trim():(!modelId || !models.some(m=>m.id===modelId))))}>
            创建
          </button>
        </DialogFooter>
      </form>

    </WorkbenchDialog>
  );
}
function DocumentTab({
  api,
  params,
}: IDockviewPanelHeaderProps<{ id: string }>) {
  const { store, closeDocument } = useWorkbench();
  const draft = api.id.startsWith("model:")
    ? store.modelDraft(params.id)
    : undefined;
  const endpoint = api.id.startsWith("endpoint:")
    ? store.endpoint(params.id)
    : undefined;
  const title = endpoint
    ? endpoint.name
    : (draft?.name ?? api.title);
  return (
    <div
      className="document-tab"
      onPointerDownCapture={(event) => {
        if (event.button === 1) {
          event.preventDefault();
          event.stopPropagation();
          closeDocument(api.id, () => api.close());
        }
      }}
    >
      {api.id.startsWith("resource:")?<BookOpen size={16}/>:endpoint ? <Table2 size={16} /> : <FileSpreadsheet size={16} />}
      <span>
        {title}
        {draft?.dirty || (endpoint && store.endpointDirty(endpoint.id)) ? " ●" : ""}
      </span>
      <button
        type="button"
        aria-label={`关闭 ${title}`}
        onPointerDown={(event) => event.stopPropagation()}
        onClick={(event) => {
          event.stopPropagation();
          closeDocument(api.id, () => api.close());
        }}
      >
        <X size={14} />
      </button>
    </div>
  );
}
function ObservationTab({ api }: IDockviewPanelHeaderProps) {
  return <span className="observation-tab">{api.title}</span>;
}
const tabComponents = { document: DocumentTab, observation: ObservationTab };
function HeaderActions({ group }: IDockviewHeaderActionsProps) {
  const { dialog } = useWorkbench();
  if (group.id === "observation-edge") {
    return <Icon label="收起底部面板" onClick={() => group.api.collapse()}><ChevronRight size={14} style={{transform:"rotate(90deg)"}} /></Icon>;
  }
  if (
    !group.panels.some(
      (panel) =>
        panel.id === "welcome" ||
        panel.id.startsWith("model:") ||
        panel.id.startsWith("endpoint:") || panel.id.startsWith("resource:"),
    )
  )
    return null;
  return (
    <Icon
      label="新建文档"
      onClick={() =>
        dialog(
          group.activePanel?.id.startsWith("model:") ? "model" : "endpoint",
        )
      }
    >
      <Plus size={20} />
    </Icon>
  );
}
export function App() {
  useEffect(() => {
    let active = true;
    let cancel: (() => void) | undefined;
    void armEditCommands((action) => {
      const panel = api.current?.activePanel;
      if (!panel?.id.startsWith("model:") || action === "selectAll")
        return false;
      const draft = store.modelDrafts.get(panel.id.slice(6));
      if (!draft) return false;
      action === "undo" ? draft.undo() : draft.redo();
      return true;
    }).then((value) => {
      if (active) cancel = value;
      else value();
    });
    return () => {
      active = false;
      cancel?.();
    };
  }, []);
  const store = useMemo(() => new WorkspaceStore(createWorkspaceClient()), []);
  const sheets = useMemo(() => new SheetViews(), []);
  useSyncExternalStore(store.subscribe, store.snapshot);
  const api = useRef<DockviewApi | null>(null);
  const activeDocument = useRef<string | null>(null);
  const resourceNavigationCount=useRef(0);
  useEffect(() => {
    let active = true;
    let cancelNative: (() => void) | undefined;
    const save = () => {
      if (hasOpenSaveDialog(document)) return;
      const button = [...document.querySelectorAll<HTMLButtonElement>("button[data-save-document]")]
        .find(item => item.dataset.saveDocument === activeDocument.current);
      if (!button || button.disabled) return;
      // 聚焦工具栏先提交NovaSheet尚在编辑的文本，再走原有保存／错误处理。
      button.focus({ preventScroll: true });
      queueMicrotask(() => { if (active && button.isConnected) button.click(); });
    };
    const cancelKeys = bindSaveShortcut(window, save);
    void armSaveCommand(save).then(cancel => {
      if (active) cancelNative = cancel;
      else cancel();
    });
    return () => { active = false; cancelKeys(); cancelNative?.(); };
  }, []);
  const [selected, setSelected] = useState<Target | null>(null);
  const [dialog, setDialog] = useState<DialogKind>(null);
  const [closeRequest, setCloseRequest] = useState<{
    id: string;
    close: () => void;
  } | null>(null);
  const [exitRequest, setExitRequest] = useState(false);
  const closeDocument = (id: string, close: () => void) => {
    const draft = id.startsWith("model:")
      ? store.modelDrafts.get(id.slice(6))
      : undefined;
    if (draft?.dirty) setCloseRequest({ id, close });
    else if(id.startsWith("endpoint:") && store.endpointDirty(id.slice(9))) setCloseRequest({id,close});
    else close();
  };
  useEffect(() => {
    let dispose: (() => void) | undefined;
    let active = true;
    void armCloseGuard(
      () =>
        !!(
          store.modelDrafts.size ||
          store.drafts.size ||
          store.view?.catalog.models.length ||
          store.view?.catalog.endpoints.length ||
          store.view?.catalog.devices.length ||
          store.view?.catalog.navigation?.folders.length
        ),
      () => setExitRequest(true),
    ).then((fn) => {
      if (active) dispose = fn;
      else fn();
    });
    return () => {
      active = false;
      dispose?.();
    };
  }, [store]);
  useEffect(() => {
    let disposed = false;
    let disconnect: (() => Promise<void>) | undefined;
    void store.connect().then((cancel) => {
      if (disposed) void cancel();
      else disconnect = cancel;
    });
    return () => {
      disposed = true;
      void disconnect?.();
    };
  }, [store]);
  const open = (target: Target) => {
    if(target.kind==="model"||target.kind==="endpoint")void store.loadTarget(target.kind,target.id).catch(e=>{store.error=errorMessage(e);store.changed();});
    setSelected((previous) =>
      previous?.id === target.id &&
      previous.kind === target.kind &&
      previous.unit === target.unit
        ? previous
        : target,
    );
    if (target.kind === "device" || target.kind === "folder") return;
    if (target.unit) store.selectUnit(target.id, target.unit);
    const id = `${target.kind}:${target.id}`;
    const existing = api.current?.getPanel(id);
    if (existing) {
      if(target.kind==="resource")existing.api.updateParameters({id:target.id,anchor:target.anchor,navigation:++resourceNavigationCount.current});
      existing.api.setActive();
      return;
    }
    const title =
      target.kind === "resource"?resources.find(resource=>resource.id===target.id)?.title:target.kind === "model"
        ? store.modelDraft(target.id)?.name
        : store.endpoint(target.id)?.name;
    api.current?.addPanel({
      id,
      component: target.kind,
      tabComponent: "document",
      params: { id: target.id,...(target.kind==="resource"?{anchor:target.anchor,navigation:++resourceNavigationCount.current}:{}) },
      title: title ?? "配置",
      renderer: "always",
      position: documentPosition(
        api.current?.panels.map((panel) => panel.id) ?? [],
      ),
    });
  };
  useEffect(()=>{
    const error=store.lastError;
    if(error?.endpointId)open({kind:"endpoint",id:error.endpointId,unit:error.unitId});
  },[store.lastError]);
  const ready = ({ api: layout }: DockviewReadyEvent) => {
    api.current = layout;
    const initial = initialWorkbenchLayout(layout.width || window.innerWidth, layout.height || window.innerHeight);
    layout.addPanel({ id: "welcome", component: "welcome", title: "工作台" });
    // 左右边栏包围中央列，使底边输出区只占文档区宽度。
    const navigation = layout.addEdgeGroup("left", {id:"navigation-edge",initialSize:initial.navigation,minimumSize:180});
    layout.addPanel({
      id: "project",
      component: "project",
      title: "导航",
      position: { referenceGroup: navigation.id, direction: "within" },
    });
    const inspector = layout.addEdgeGroup("right", {id:"inspector-edge",initialSize:initial.inspector,minimumSize:220});
    layout.addPanel({
      id: "inspector",
      component: "inspector",
      title: "属性",
      position: { referenceGroup: inspector.id, direction: "within" },
    });
    // 保留原生 edge 的收起、展开与拖动高度，输出面板位于中央列下方。
    const edge = layout.addEdgeGroup("bottom", {
      id: "observation-edge", initialSize: initial.output, minimumSize: 120,
      collapsedSize: 26, collapsed: false,
    });
    edge.setHeaderPosition("bottom");
    for (const [id, component, title, params] of [
      ["observations", "observations", "事件记录", {view:"events"}],
      ["messages", "observations", "通信报文", {view:"messages"}],
      ["problems", "problems", "问题", {}],
    ] as const) {
      layout.addPanel({id, component, title, params, tabComponent:"observation",
        inactive:true, position:{referenceGroup:edge.id, direction:"within"}});
    }
    layout.getPanel("messages")?.api.setActive();
    layout.getPanel("welcome")?.api.setActive();
    // 使用Dockview的公开隐藏接口同步重算内容高度，避免只隐藏DOM留出空白标题行。
    for (const id of ["project", "inspector"]) {
      const panel = layout.getPanel(id);
      if (panel) panel.group.model.header.hidden = true;
    }
    layout.onDidActivePanelChange(({ panel }) => {
      if (panel?.id.startsWith("endpoint:") || panel?.id.startsWith("model:")) activeDocument.current = panel.id;
      if(panel?.id.startsWith("resource:")){activeDocument.current=null;setSelected({kind:"resource",id:panel.id.slice(9)});}
      else if (panel?.id.startsWith("endpoint:"))
        setSelected({ kind: "endpoint", id: panel.id.slice(9) });
      else if (panel?.id.startsWith("model:"))
        setSelected({ kind: "model", id: panel.id.slice(6) });
    });
  };
  const components = useMemo(
    () => ({
      resource:({params}:IDockviewPanelProps<{id:string;anchor?:string;navigation?:number}>)=><ResourceDocument {...params}/>,
      resources:()=>{const {store}=useWorkbench();return <ResourcePanel store={store}/>;},
      batch:()=>{const {store,sheets}=useWorkbench();return <BatchPanel store={store} sheets={sheets}/>;},
      project: ProjectPanel,
      inspector: InspectorPanel,
      welcome: Welcome,
      endpoint: (props: IDockviewPanelProps<{ id: string }>) => (
        <EndpointPanel params={props.params} api={props.api} />
      ),
      model: (props: IDockviewPanelProps<{ id: string }>) => (
        <ModelPanel params={props.params} />
      ),
      observations: Observations,
      problems:ProblemsPanel,
    }),
    [],
  );
  return (
    <TooltipProvider delayDuration={300}>
      <Context.Provider
        value={{
          store,
          openTool:tool=>{const id=`tool:${tool}`;const existing=api.current?.getPanel(id);if(existing){existing.api.setActive();return;}api.current?.addPanel({id,component:tool,tabComponent:"document",params:{id:tool},title:tool==="batch"?"批量任务":"运行资源",position:documentPosition(api.current?.panels.map(p=>p.id)??[])});},
          sheets,
          open,
          selected,
          dialog: setDialog,
          closeDocument,
          removedEndpoint: (id) => {api.current?.getPanel(`endpoint:${id}`)?.api.close();setSelected(previous=>previous?.kind==="endpoint" && previous.id===id ? null : previous);},
          removedModel: (id) => {
            api.current?.getPanel(`model:${id}`)?.api.close();
            setSelected((current) =>
              current?.kind === "model" && current.id === id ? null : current,
            );
          },
        }}
      >
        <div className="workbench">
          {store.error && (
            <div className="error-banner" role="alert">
              {store.error}{store.lastError?.relatedUnitId && `（冲突Unit：${store.lastError.relatedUnitId}）`}
            </div>
          )}
          <main>
            <DockviewReact
              components={components}
              tabComponents={tabComponents}
              rightHeaderActionsComponent={HeaderActions}
              onReady={ready}
              theme={workbenchTheme}
            />
          </main>
          <footer aria-label="工作台状态栏">
            <span className="footer-status" title={store.view ? "已连接本次会话的配置服务" : "配置服务尚未连接"}>
              <CircleCheck size={12} className={store.view ? "connected-icon" : "muted"}/>
              {store.view ? "已连接" : "未连接"}
            </span>
            <span className="footer-status">端点 {store.view?.catalog.endpoints.length ?? 0}</span>
            <span className="footer-status" title="已保存、尚未显式应用的端点">
              待应用 {store.view?.catalog.endpoints.filter(e => !isEndpointApplied(store.view!.catalog,e,store.view!.applicationMatches)).length ?? 0}
            </span>
            <button className={`footer-status ${store.error ? "error" : ""}`} title="展开问题面板" onClick={()=>{
              api.current?.getPanel("problems")?.api.setActive();
              api.current?.getEdgeGroup("bottom")?.expand();
            }}>问题 {[...store.modelDrafts.values()].reduce((n,d)=>n+(d.preview?.errors.length??0),store.error?1:0)}</button>
            <span className="spacer"/>
            <span className="footer-session" title="当前配置仅保留本次会话，退出即丢失">会话配置 · 退出不保留</span>
          </footer>
        </div>
        {closeRequest && (
          <CloseDraftDialog
            store={store}
            request={closeRequest}
            cancel={() => setCloseRequest(null)}
          />
        )}
        {exitRequest && <ExitDialog cancel={() => setExitRequest(false)} />}
        {dialog && <CreateDialog kind={dialog} close={() => setDialog(null)} />}
      </Context.Provider>
    </TooltipProvider>
  );
}
