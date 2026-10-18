import { NavigationDialog, type NavigationRequest } from "../features/workspace/NavigationDialog";
import { EnumSelect } from "../features/workspace/EnumSelect";
import { bindSaveShortcut, hasOpenSaveDialog } from "../features/workspace/saveShortcut";
import { nextStation, applicationStateLabel, isEndpointApplied } from "../features/workspace/model";
import { ApplyModelDialog } from "../features/workspace/ApplyModelDialog";
import {
  createContext,
  useContext,
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
import { FileTree, useFileTree } from "@pierre/trees/react";
import { NovaSheetGrid } from "@zhiguang/novasheet-react";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@essim/ui/components/tooltip";
import {
  Plus,
  Save,
  Play,
  Check,
  MoreHorizontal,
  Table2,
  Server,
  Box,
  RefreshCw,
  ChevronLeft,
  ChevronRight,
  Menu,
  Search,
  ListFilter,
  FolderOpen,
  X,
  CircleCheck,
  Info,
  FileCheck2,
  FileSpreadsheet,
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
  navigation,
  documentPosition,
  type Target,
} from "../features/workspace/model";
import {
  SheetViews,
  resolveUnitSheet,
  categories,
  readonlyTypes,
  sheetTheme,
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
  const { store, open, dialog, removedModel, removedEndpoint } = useWorkbench();
  const [section, setSection] = useState<"resources" | "simulators">(
    "simulators",
  );
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
  const [navigationAction,setNavigationAction]=useState<NavigationRequest|null>(null);
  const [toolsOpen, setToolsOpen] = useState(false);
  const [importing, setImporting] = useState(false);
  const [appendImport, setAppendImport] = useState<string | null>(null);
  const [resourceAction, setResourceAction] = useState<{
    kind: "rename" | "copy" | "delete";
    id: string;
  } | null>(null);
  const [rootMenu, setRootMenu] = useState<{ x: number; y: number } | null>(
    null,
  );
  const input = useMemo(
    () =>
      catalog
        ? navigation(catalog, section)
        : { paths: [], targets: new Map<string, Target>() },
    [catalog, section],
  );
  const current = useRef({ input, open });
  current.current = { input, open };
  const { model } = useFileTree({
    paths: [],
    initialExpansion: "open",
    flattenEmptyDirectories: false,
    density: "default",
    itemHeight: 30,
    search: false,
    renaming: false,
    dragAndDrop: false,
    composition: { contextMenu: { enabled: true, triggerMode: "both" } },
    onSelectionChange: (paths) => {
      const target = current.current.input.targets.get(paths.at(-1) ?? "");
      if (target) current.current.open(target);
    },
    unsafeCSS:
      ":host { color-scheme: light; --trees-bg-override:#fff; --trees-fg-override:#27334b; --trees-font-size-override:13px; --trees-selected-bg-override:#e7eef4; --trees-selected-focused-border-color-override:transparent; --trees-accent-override:#126bff; --trees-indent-guide-bg-override:transparent; }",
  });
  useEffect(() => {
    model.resetPaths(input.paths);
  }, [model, input]);
  return (
    <div
      className="panel column navigation-panel"
      tabIndex={0}
      aria-label="资源导航面板"
      onContextMenu={(event) => {
        if (section === "resources" && !event.defaultPrevented) {
          event.preventDefault();
          setRootMenu({ x: event.clientX, y: event.clientY });
        }
      }}
      onKeyDown={(event) => {
        if (
          !event.defaultPrevented &&
          event.target === event.currentTarget &&
          event.shiftKey &&
          event.key === "F10" &&
          section === "resources"
        ) {
          event.preventDefault();
          setRootMenu({ x: 24, y: 120 });
        }
      }}
    >
      <div className="tree-tabs">
        <button
          className={section === "resources" ? "active" : ""}
          onClick={() => setSection("resources")}
        >
          资源
        </button>
        <button
          className={section === "simulators" ? "active" : ""}
          onClick={() => setSection("simulators")}
        >
          模拟器
        </button>
        <span className="spacer" />
        <Icon label="新建资源工具" onClick={() => setToolsOpen(!toolsOpen)}>
          <MoreHorizontal size={17} />
        </Icon>
        <Icon
          label={section === "resources" ? "新建采集模型" : "新建模拟器"}
          disabled={!store.view || store.busy}
          onClick={() => dialog(section === "resources" ? "model" : "endpoint")}
        >
          <Plus size={15} />
        </Icon>
      </div>
      {(section === "resources" || toolsOpen) && (
        <div className="tree-actions">
          {section === "resources" ? (
            <>
              <button
                disabled={!store.view || store.busy}
                onClick={() => dialog("model")}
              >
                <Table2 size={13} /> 采集模型
              </button>
              <button
                disabled={!store.view || store.busy}
                onClick={() => dialog("device")}
              >
                <Box size={13} /> 设备
              </button>
            </>
          ) : (
            <button
              disabled={!store.view || store.busy}
              onClick={() => dialog("endpoint")}
            >
              <Server size={13} /> 新建端点／通道
            </button>
          )}
        </div>
      )}
      {input.paths.length ? (
        <FileTree
          model={model}
          style={{ flex: 1, minHeight: 0 }}
          renderContextMenu={(item, context) => {
            const target = input.targets.get(String(item.path));
            const act = (action: () => void) => {
              context.close({ restoreFocus: false });
              action();
            };
            return (
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
              </div>
            );
          }}
        />
      ) : (
        <Empty>
          {store.view ? "尚无配置，请点击上方新建。" : "等待连接 Rust 核心。"}
        </Empty>
      )}
      {section==="simulators" && <button onClick={()=>setNavigationAction({kind:"newFolder"})} disabled={!store.view || store.busy}>新建文件夹…</button>}
      {navigationAction && <NavigationDialog store={store} request={navigationAction} close={()=>setNavigationAction(null)} open={open} removed={removedEndpoint} />}
      <div className="navigation-note">本次会话 · 退出后不保留配置和目录</div>
      {rootMenu && (
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
            <button
              role="menuitem"
              onClick={() => {
                setRootMenu(null);
                dialog("model");
              }}
            >
              新建采集模型
            </button>
            <button
              role="menuitem"
              onClick={() => {
                setRootMenu(null);
                setImporting(true);
              }}
            >
              导入新模型…
            </button>
          </div>
        </div>
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
    <Empty>
      <Table2 size={30} />
      <h2>Modbus 工作台</h2>
      <p>创建采集模型，再创建模拟器并添加单元。</p>
      <div className="welcome-actions">
        <button
          disabled={!store.view || store.busy}
          onClick={() => dialog("model")}
        >
          新建采集模型
        </button>
        <button
          disabled={!store.view || store.busy}
          onClick={() => dialog("endpoint")}
        >
          新建模拟器
        </button>
      </div>
      <p>当前提供配置骨架，正式通信、可靠录制与回放待接入。</p>
    </Empty>
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
  useEffect(() => {
    ref.current?.showModal();
  }, []);
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
    <dialog ref={ref} onCancel={close}>
      <h2>
        {action.kind === "rename"
          ? "重命名模型草稿"
          : action.kind === "copy"
            ? "复制模型"
            : "删除模型"}
      </h2>
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
      <div className="dialog-actions">
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
      </div>
    </dialog>
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
function EndpointPanel({ params }: { params: { id: string } }) {
  const { store, sheets } = useWorkbench();
  const endpoint = store.endpoint(params.id);
  const [more, setMore] = useState(false);
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
  const reason = store.view?.capabilities.reason ?? "核心未连接";
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
  return (
    <div className="panel column">
      <div className="toolbar">
        <Icon
          label="保存配置到本次会话（⌘S / Ctrl+S）"
          saveDocument={`endpoint:${endpoint.id}`}
          disabled={store.busy || !store.drafts.has(endpoint.id)}
          onClick={() =>
            attempt(
              store.execute({
                kind: "saveEndpoint",
                endpoint,

              }),
            )
          }
        >
          <Save size={17} />
        </Icon>
        <Icon
          label={
            store.drafts.has(endpoint.id)
              ? "请先保存草稿，再应用配置"
              : "应用已保存配置"
          }
          disabled={
            store.busy ||
            !saved ||
            store.drafts.has(endpoint.id) ||
            isEndpointApplied(store.view?.catalog, saved, store.view?.applicationMatches)
          }
          onClick={() => setApplying(true)}
        >
          <FileCheck2 size={20} />
        </Icon>
        <i />
        <Icon label={`启动不可用：${reason}`} disabled>
          <Play size={17} />
        </Icon>
        <i />
        <Icon
          label={`${endpoint.role === "master" ? "下发遥控／遥调" : "提交遥测／遥信置数"}不可用：${reason}`}
          disabled
        >
          <Check size={17} />
        </Icon>
        <Icon label="更多工具" onClick={() => setMore(!more)}>
          <MoreHorizontal size={18} />
        </Icon>
        <span className="spacer" />
        <span className="run-status">
          <span className="status-dot" />
          {applicationStateLabel(store.view?.applicationStates?.[endpoint.id])}
        </span>
        <span>{isEndpointApplied(store.view?.catalog, endpoint, store.view?.applicationMatches) ? "已应用" : applied ? "待应用" : "未应用"}</span>
      </div>
      {resetting && unit && <ConfirmAction title={`恢复 ${unit.name} 初值`} message={endpoint.role === "slave" ? "仅恢复选定Unit已应用模型的初值，不应用未保存或待应用修改。" : "仅清空选定Unit本地缓存并标记过期，不向远端写入。"} close={()=>setResetting(false)} submit={()=>store.execute({kind:"resetUnit",id:endpoint.id,unit:unit.id})} />}
      {applying && (
        <ApplyModelDialog
          store={store}
          endpoint={saved ?? endpoint}
          close={() => setApplying(false)}
        />
      )}
      {more && (
        <div className="more-tools" role="menu">
          <button disabled={!unit || !store.view?.catalog.values[unit.id] || store.busy} onClick={()=>setResetting(true)}>恢复选定Unit初值…</button>
          {["批量置数", "导出", "录制", "回放"].map((label) => (
            <button key={label} disabled title="后续阶段接入">
              {label} · 未接入
            </button>
          ))}
        </div>
      )}
      <div className="model-strip visually-compact-metadata">
        <span>
          {unit ? `Unit ${unit.station} · ${unit.name}` : "未添加单元"}
        </span>
        <span className="spacer" />
        <span>
          {model
            ? model.name
            : "请在属性栏添加单元并选择模型"}
        </span>
      </div>
      {unit && store.modelDrafts.get(unit.model.id)?.dirty && (
        <div className="panel-note">引用模型有未保存修改，请先保存模型；当前显示已保存点表。</div>
      )}
      <div className="sheet-area">
        {!unit ? (
          <Empty>在右侧属性栏添加 Unit，并引用一个已保存的采集模型。</Empty>
        ) : (
          <>
            {visited.map((id) => {
              const viewUnit = endpoint.units.find((item) => item.id === id);
              if (!viewUnit) return null;
              const { model: viewModel, values: viewValues } = resolveUnitSheet(
                store.view?.catalog, endpoint, viewUnit, store.drafts.has(endpoint.id) ? {[endpoint.id]:store.draftMatches.get(endpoint.id) ?? store.view?.applicationMatches?.[endpoint.id] ?? {}} : store.view?.applicationMatches,
              );
              return categories.map(([value, label]) => (
                <div
                  key={`${id}:${value}`}
                  className="sheet-slot"
                  style={{
                    display:
                      id === key && value === category ? "block" : "none",
                  }}
                >
                  <NovaSheetGrid
                    excelHeaders
                    excelWorkspace
                    rowHeaderCornerLabel=""
                    data={sheets.get(
                      id,
                      value,
                      viewModel,
                      viewValues,
                      endpoint.role,
                    )}
                    theme={sheetTheme}
                    cellTypes={readonlyTypes}
                    className="h-full w-full"
                    interactions={{
                      contextMenu: false,
                      reorder: false,
                      resize: { rows: false, columns: true },
                    }}
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
      <div className="category-tabs" role="tablist" aria-label="四遥分类">
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
 useEffect(()=>{ref.current?.showModal();},[]);
 return <dialog ref={ref} onCancel={e=>{e.preventDefault();e.stopPropagation();if(!busy)close();}}><h2>{title}</h2><p>{message}</p>{error && <p role="alert" className="error">{error}</p>}<div className="dialog-actions"><button disabled={busy} onClick={close}>取消</button><button disabled={busy} onClick={async()=>{setBusy(true);try{await submit();close();}catch(e){setError(errorMessage(e));}finally{setBusy(false);}}}>确认</button></div></dialog>;
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
  useEffect(() => {
    ref.current?.showModal();
  }, []);
  const endpointId = request.id.startsWith("endpoint:") ? request.id.slice(9) : null;
  const modelId = request.id.startsWith("model:") ? request.id.slice(6) : request.id;
  const finish = () => {
    request.close();
    cancel();
  };
  return (
    <dialog ref={ref} onCancel={e=>{e.preventDefault();e.stopPropagation();if(!busy)cancel();}}>
      <h2>{endpointId ? "模拟器" : "模型"}有未保存修改</h2>
      <p>保存仅保留本次会话。请选择如何关闭。</p>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      <div className="dialog-actions">
        <button disabled={busy} onClick={cancel}>
          取消
        </button>
        <button
          disabled={busy}
          onClick={() => {
            if(endpointId) store.drafts.delete(endpointId);
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
                await store.execute({kind:"saveEndpoint",endpoint});
              } else await store.saveModelDraft(modelId);
              if (endpointId ? store.drafts.has(endpointId) : store.modelDrafts.get(modelId)?.dirty) {
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
      </div>
    </dialog>
  );
}
function ExitDialog({ cancel }: { cancel: () => void }) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    ref.current?.showModal();
  }, []);
  return (
    <dialog ref={ref} onCancel={cancel}>
      <h2>退出本次会话？</h2>
      <p>当前所有配置（包括已保存配置）、文件夹结构及未保存草稿，退出后都会丢失。</p>
      <div className="dialog-actions">
        <button onClick={cancel}>取消退出</button>
        <button onClick={() => void closeWorkbenchWindow()}>放弃并退出</button>
      </div>
    </dialog>
  );
}

const PropertySearch = createContext("");
function Property({ label, children, field }: { label: string; children: ReactNode; field?: string }) {
  const search = useContext(PropertySearch);
  if (search && !label.toLowerCase().includes(search.toLowerCase()))
    return null;
  return (
    <label className="property" data-config-field={field}>
      <span>{label}</span>
      <div>{children}</div>
    </label>
  );
}
function InspectorPanel() {
  const { store, selected, open } = useWorkbench();
  const [search, setSearch] = useState("");
  const searchInput = useRef<HTMLInputElement>(null);
  const inspector=useRef<HTMLDivElement>(null);
  const endpoint =
    selected?.kind === "endpoint" ? store.endpoint(selected.id) : undefined;
  const unit = currentUnit(store, endpoint);
  const catalog = store.view?.catalog;
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
    <div ref={inspector} className="panel details inspector-panel">
      <div className="details-title">
        <Menu size={16} />
        <strong>详细信息</strong>
        <span className="spacer" />
        <span className="details-save-state">
          {endpoint && store.drafts.has(endpoint.id) ? "未保存" : ""}
        </span>
        <MoreHorizontal size={17} className="muted" />
      </div>
      <div className="details-subject">
        {endpoint
          ? `${endpoint.transport.kind.toUpperCase()} ${endpoint.role === "slave" ? "从站" : "主站"}${unit ? ` / Unit ${unit.station}` : ""}`
          : (device?.name ?? "未选择对象")}
      </div>
      <div className="property-search">
        <Search size={17} />
        <input
          ref={searchInput}
          aria-label="搜索属性"
          placeholder="搜索属性"
          value={search}
          onChange={(event) => setSearch(event.target.value)}
        />
        <Icon
          label="按属性名称搜索"
          onClick={() => searchInput.current?.focus()}
        >
          <ListFilter size={17} />
        </Icon>
      </div>
      <PropertySearch.Provider value={search}>
        {!endpoint ? (
          device ? (
            <details open>
              <summary>设备</summary>
              <Property label="名称" field="unit.name">{device.name}</Property>
              <Property label="类型">{device.kind}</Property>
            </details>
          ) : (
            <Empty>选择模拟器或设备查看属性。</Empty>
          )
        ) : (
          <>
            <details open>
              <summary>单元</summary>
              <Property label="当前单元" field="units">
                <div className="unit-select">
                  <EnumSelect label="选择单元" value={unit?.id ?? ""} onChange={v=>store.selectUnit(endpoint.id,v)} options={[["","尚无单元"],...endpoint.units.map(u=>[u.id,`${u.name} · Unit ${u.station}${u.enabled ? "" : " · 已禁用"}`] as const)]} />
                  <Icon
                    label={
                      canAdd
                        ? "添加 Unit"
                        : "请先创建采集模型；RTU从站仅允许一个单元"
                    }
                    disabled={!canAdd || store.busy}
                    onClick={add}
                  >
                    <Plus size={19} />
                  </Icon>
                </div>
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
                      value={unit.name}
                      onChange={(event) => patch({ name: event.target.value })}
                    />
                  </Property>
                  <Property label="Unit备注"><input value={unit.note ?? ""} onChange={e=>patch({note:e.target.value})} /></Property>
                  <Property label="删除单元"><button onClick={()=>edit({...endpoint,units:endpoint.units.filter(u=>u.id!==unit.id)})}>从草稿移除 {unit.name}</button></Property>
                  <Property label="启用">
                    <input
                      type="checkbox"
                      checked={unit.enabled}
                      onChange={(event) =>
                        patch({ enabled: event.target.checked })
                      }
                    />
                  </Property>
                  <Property label="关联设备" field="device">
                    <div className="reference-field">
                      <EnumSelect label="关联设备" value={unit.device ?? ""} onChange={v=>patch({device:v || null})} options={[["","独立模拟器"],...(catalog?.devices ?? []).map(d=>[d.id,d.name] as const)]} />
                      <Icon
                        label="定位关联设备"
                        disabled={!unit.device}
                        onClick={() =>
                          unit.device &&
                          open({ kind: "device", id: unit.device })
                        }
                      >
                        <FolderOpen size={17} />
                      </Icon>
                      <Icon
                        label="解除设备关联"
                        disabled={!unit.device}
                        onClick={() => patch({ device: null })}
                      >
                        <X size={16} />
                      </Icon>
                    </div>
                  </Property>
                  <Property label="采集模型" field="model">
                    <div className="reference-field">
                      <EnumSelect label="采集模型" value={unit.model.id} onChange={v=>patch({model:{id:v}})} options={latest.map(m=>[m.id,m.name] as const)} />
                      <Icon
                        label="打开采集模型"
                        onClick={() =>
                          open({ kind: "model", id: unit.model.id })
                        }
                      >
                        <FolderOpen size={17} />
                      </Icon>
                    </div>
                  </Property>
                </>
              )}
            </details>
            <details open>
              <summary>所属端点</summary>
              <Property label="模拟器备注"><input value={endpoint.note ?? ""} onChange={e=>edit({...endpoint,note:e.target.value})} /></Property>
              <Property label="角色">
                {endpoint.role === "slave" ? "从站" : "主站"}
              </Property>
              <Property label="协议">
                Modbus {endpoint.transport.kind.toUpperCase()}
              </Property>
              <Property label="名称" field="name">
                <input
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
                    <input
                      value={endpoint.transport.path}
                      onChange={(event) => {
                        if (endpoint.transport.kind === "rtu")
                          edit({
                            ...endpoint,
                            transport: {
                              ...endpoint.transport,
                              path: event.target.value,
                            },
                          });
                      }}
                    />
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
            </details>
            <details>
              <summary>高级</summary>
              <div className="panel-note">
                {store.view?.capabilities.reason}
              </div>
            </details>
          </>
        )}
      </PropertySearch.Provider>
      <div className="details-hint">切换单元仅切换视图，不影响运行与草稿。</div>
    </div>
  );
}
function Observations({ params }: { params?: { view?: string } }) {
  const kind = params?.view ?? "events";
  const columns =
    kind === "messages"
      ? ["时间", "方向", "端点", "内容"]
      : ["时间", "对象", "事件", "结果"];
  return (
    <div className="panel column observation-panel">
      <div className="observation-columns">
        {columns.map((label) => (
          <span key={label}>{label}</span>
        ))}
      </div>
      <Empty>{kind === "messages" ? "暂无通信报文" : "暂无事件记录"}</Empty>
    </div>
  );
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
  const [creatingModel,setCreatingModel]=useState(false);
  useEffect(()=>{if(!modelId && models.length===1) setModelId(models[0].id);},[models,modelId]);
  useEffect(() => {
    store.error="";store.lastError=null;store.changed();
    ref.current?.showModal();
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
          units: [{id:crypto.randomUUID(),name:unitName,station:Number(station),enabled:true,model:{id:modelId},device:null}],
        };
        await store.execute({
          kind: "createEndpoint",
          endpoint,

        });
        open({ kind: "endpoint", id });
      }
      close();
    } catch {
      /* 错误由真实应用响应展示，保留草稿供修正。 */
    }
  }
  return (
    <dialog ref={ref} onCancel={e=>{e.preventDefault();e.stopPropagation();if(!store.busy)close();}} aria-label={title}>
      <form onSubmit={submit}>
        <h2>{title}</h2>
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
            <label>采集模型<EnumSelect label="首个Unit模型" value={modelId} onChange={setModelId} options={[["","请选择已保存模型"],...models.map(m=>[m.id,m.name] as const)]} /></label>
            {models.length===0 && <p>请先创建采集模型。<button type="button" onClick={()=>setCreatingModel(true)}>创建模型并返回</button></p>}
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
                <label>
                  串口路径
                  <input
                    required
                    placeholder="填写实际串口路径"
                    value={path}
                    onChange={(event) => setPath(event.target.value)}
                  />
                </label>
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
        <div className="dialog-actions">
          <button type="button" disabled={store.busy} onClick={close}>
            取消
          </button>
          <button className="primary" disabled={store.busy || !name.trim() || (kind === "endpoint" && (!modelId || !models.some(m=>m.id===modelId)))}>
            创建
          </button>
        </div>
      </form>
      {creatingModel && <CreateDialog kind="model" close={()=>setCreatingModel(false)} />}
    </dialog>
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
      {endpoint ? <Table2 size={16} /> : <FileSpreadsheet size={16} />}
      <span>
        {title}
        {draft?.dirty || (endpoint && store.drafts.has(endpoint.id)) ? " ●" : ""}
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
  if (
    !group.panels.some(
      (panel) =>
        panel.id === "welcome" ||
        panel.id.startsWith("model:") ||
        panel.id.startsWith("endpoint:"),
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
    else if(id.startsWith("endpoint:") && store.drafts.has(id.slice(9))) setCloseRequest({id,close});
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
      existing.api.setActive();
      return;
    }
    const title =
      target.kind === "model"
        ? store.modelDraft(target.id)?.name
        : store.endpoint(target.id)?.name;
    api.current?.addPanel({
      id,
      component: target.kind,
      tabComponent: "document",
      params: { id: target.id },
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
    layout.addPanel({ id: "welcome", component: "welcome", title: "工作台" });
    layout.addPanel({
      id: "project",
      component: "project",
      title: "导航",
      position: { referencePanel: "welcome", direction: "left" },
      initialWidth: 260,
    });
    layout.addPanel({
      id: "inspector",
      component: "inspector",
      title: "属性",
      position: { referencePanel: "welcome", direction: "right" },
      initialWidth: 350,
    });
    layout.addPanel({
      id: "observations",
      component: "observations",
      title: "事件记录",
      tabComponent: "observation",
      position: { referencePanel: "welcome", direction: "below" },
      initialHeight: 150,
    });
    layout.addPanel({
      id: "messages",
      component: "observations",
      title: "通信报文",
      tabComponent: "observation",
      params: { view: "messages" },
      inactive: true,
      position: { referencePanel: "observations", direction: "within" },
    });
    layout.getPanel("observations")?.api.setActive();
    // 使用Dockview的公开隐藏接口同步重算内容高度，避免只隐藏DOM留出空白标题行。
    for (const id of ["project", "inspector"]) {
      const panel = layout.getPanel(id);
      if (panel) panel.group.model.header.hidden = true;
    }
    layout.onDidActivePanelChange(({ panel }) => {
      if (panel?.id.startsWith("endpoint:") || panel?.id.startsWith("model:")) activeDocument.current = panel.id;
      if (panel?.id.startsWith("endpoint:"))
        setSelected({ kind: "endpoint", id: panel.id.slice(9) });
      else if (panel?.id.startsWith("model:"))
        setSelected({ kind: "model", id: panel.id.slice(6) });
    });
  };
  const components = useMemo(
    () => ({
      project: ProjectPanel,
      inspector: InspectorPanel,
      welcome: Welcome,
      endpoint: (props: IDockviewPanelProps<{ id: string }>) => (
        <EndpointPanel params={props.params} />
      ),
      model: (props: IDockviewPanelProps<{ id: string }>) => (
        <ModelPanel params={props.params} />
      ),
      observations: Observations,
    }),
    [],
  );
  return (
    <TooltipProvider delayDuration={300}>
      <Context.Provider
        value={{
          store,
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
          <header>
            <Box size={22} className="brand-icon" />
            <strong>ESSim</strong>
            <span className="app-title">Modbus 协议仿真工作台</span>
            <span className="spacer" />
            <Icon
              label="刷新真实配置目录"
              onClick={() =>
                attempt(
                  store.refresh().catch((error) => {
                    store.error = String(error.message ?? error);
                    store.changed();
                  }),
                )
              }
            >
              <RefreshCw size={14} />
            </Icon>
            <span className="muted">本次会话</span>
          </header>
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
          <footer>
            <span className="footer-status">
              <CircleCheck
                size={17}
                className={store.view ? "connected-icon" : "muted"}
              />
              {store.view ? "配置工作台已连接" : "核心未连接"}
            </span>
            <i />
            <span className="footer-status">
              <span className="status-dot" />
              当前无运行任务
            </span>
            <i />
            <span className="footer-status">
              <Info size={17} />
              录制失败将停止对应运行
            </span>
            <span className="spacer" />
            <span className="muted">本次会话 · 配置退出即丢失</span>
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
