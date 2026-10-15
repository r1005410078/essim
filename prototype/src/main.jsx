import React, {
  createContext,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";
import { createRoot } from "react-dom/client";
import { DockviewReact, themeLight } from "dockview-react";
import "dockview-react/dist/styles/dockview.css";
import {
  Box,
  Code2,
  Plus,
  Save,
  Play,
  Square,
  RotateCcw,
  Upload,
  Undo2,
  Redo2,
  Download, Plug, Unplug, Pencil, ListPlus, Trash2, RefreshCw, ArrowDownToLine, ArrowUpFromLine,
  FilePlus2,
  X,
} from "lucide-react";
import { FileTree, useFileTree } from "@pierre/trees/react";
import CodeMirror from "@uiw/react-codemirror";
import { javascript } from "@codemirror/lang-javascript";
import { NovaSheetGrid } from "@zhiguang/novasheet-react";
import { InMemoryDataSource, denseGridTheme } from "@zhiguang/novasheet-core";
import ModelView from "./ModelView";
import {
  types,
  protocols,
  validateDevice,
  validateComm,
  blankTemplate,
  blankComm,
  demoPoints,
  demoWorkspace,
  migrateProject,
  endpointLabel,
  commLabel,
  templatePoints,
  templateDeviceNames,
  peersFor,
  pointFields,
} from "./domain";
import {
  categories,
  isTelemetry,
  pointCategory,
  validateAction,
  findPeerPoint,
} from "./telecontrol";
import { ensureSpareRows, isBlankPointRow } from "./pointSheet";
import { defaultScript } from "./simulation";
import {
  buildResourceTree,
  buildProjectFileTree,
  canonicalTreePath,
  modbusBadgeSvg,
  resourceTreeCss,
  projectTreeCss,
  resourceTreeIcons,
  sanitizeTreeLabel,
  selectionPath,
  visibleTreeLabel,
} from "./treeModel";
import { folderDescendants, exportProjectFile, withDefaultFolders } from "./projectFiles";
import { prepareRuntimeBatch, runtimeKey } from "./runtimeSheet";
import "./style.css";
const Context = createContext();
const useLab = () => useContext(Context);
const numberedGridTheme = { ...denseGridTheme, metrics: { ...denseGridTheme.metrics, rowHeaderWidth: 38 } };
const key = "essim-dock-project-v3";
const params = {
  soc: 50,
  lower: 20,
  upper: 90,
  maxPower: 250,
  capacity: 1000,
};
const scripts = [
  { id: "peak", name: "peak_valley.js", source: defaultScript },
  {
    id: "demand",
    name: "demand_control.js",
    source: `// 恒功率演示，可编辑自己的需量策略\nfunction on_tick(ctx) {\n  ctx.command('PCS_01.目标功率', 80);\n}`,
  },
  {
    id: "backflow",
    name: "anti_backflow.js",
    source: `// 防逆流脚本入口，需要自行绑定负荷数据\nfunction on_tick(ctx) {\n  ctx.command('PCS_01.目标功率', 0);\n}`,
  },
];
function readProject() {
  try {
    const raw = localStorage.getItem(key) || localStorage.getItem("esslab-dock-project-v3");
    if (raw) {
      const saved = JSON.parse(raw);
      if (
        Array.isArray(saved?.devices) &&
        saved.scripts?.length &&
        Array.isArray(saved.templates)
      )
        return {
          ...withDefaultFolders(migrateProject(saved)),
          scripts: saved.scripts,
          params: saved.params || params,
        };
    }
  } catch {}
  return { ...withDefaultFolders(demoWorkspace()), scripts, params };
}
function nodeId(sel) {
  if (sel.focus === "template") return `template-${sel.templateId}`;
  if (sel.focus === "comm") return `comm-${sel.commId}`;
  return `device-${sel.deviceId}`;
}
function AppMenu({label,items}) {
  const [open,setOpen]=useState(false);const root=useRef();
  useEffect(()=>{if(!open)return;const close=e=>{if(!root.current?.contains(e.target))setOpen(false);};const escape=e=>{if(e.key==='Escape')setOpen(false);};document.addEventListener('pointerdown',close);document.addEventListener('keydown',escape);return()=>{document.removeEventListener('pointerdown',close);document.removeEventListener('keydown',escape);};},[open]);
  return <div className="app-menu" ref={root}><button aria-haspopup="menu" aria-expanded={open} onClick={()=>setOpen(v=>!v)}>{label}</button>{open&&<div className="app-menu-popup" role="menu">{items.map(({label,run,disabled})=><button key={label} role="menuitem" disabled={disabled} onClick={()=>{setOpen(false);run();}}>{label}</button>)}</div>}</div>;
}
function App() {
  const [project, setProject] = useState(readProject),
    [mode, setMode] = useState("device"),
    [selection, setSelection] = useState({
      focus: "device",
      deviceId: project.devices[0]?.id,
      commId: null,
      templateId: null,
    }),
    [scriptId, setScriptId] = useState("peak"),
    [models, setModels] = useState({}),
    [runtime, setRuntime] = useState({}),
    [simulatorSessions, setSimulatorSessions] = useState({}),
    [modal, setModal] = useState(false),
    [message, setMessage] = useState("配置原型 · 未启动通信"),
    [logs, setLogs] = useState([
      {
        time: "—",
        level: "信息",
        message: "工作区已载入，真实协议引擎尚未接入",
      },
    ]),
    [result, setResult] = useState(null),
    [running, setRunning] = useState(false);
  const dock = useRef(),
    subscription = useRef(),
    worker = useRef(),
    timer = useRef();
  const device = project.devices.find((d) => d.id === selection.deviceId),
    comm = device?.comms.find((c) => c.id === selection.commId),
    template = project.templates.find((t) => t.id === selection.templateId),
    script =
      project.scripts.find((s) => s.id === scriptId) || project.scripts[0];
  const notify = (text, level = "信息") => {
    setMessage(text);
    setLogs((l) => [
      ...l.slice(-199),
      {
        time: new Date().toLocaleTimeString("zh-CN", { hour12: false }),
        level,
        message: text,
      },
    ]);
  };
  function saveProject(p = project) {
    try {
      localStorage.setItem(key, JSON.stringify(p));
      return true;
    } catch {
      notify("本地保存失败，请检查存储空间", "错误");
      return false;
    }
  }
  const updateDevice = (id, patch) =>
    setProject((p) => ({
      ...p,
      devices: p.devices.map((d) => (d.id === id ? { ...d, ...patch } : d)),
    }));
  const updateTemplate = (id, patch) =>
    setProject((p) => ({
      ...p,
      templates: p.templates.map((t) => (t.id === id ? { ...t, ...patch } : t)),
    }));
  const updateComm = (deviceId, commId, patch) =>
    setProject((p) => ({
      ...p,
      devices: p.devices.map((d) =>
        d.id === deviceId
          ? {
              ...d,
              comms: d.comms.map((c) =>
                c.id === commId ? { ...c, ...patch } : c,
              ),
            }
          : d,
      ),
    }));
  const stop = () => {
    worker.current?.terminate();
    worker.current = null;
    clearTimeout(timer.current);
    setRunning(false);
  };
  useEffect(
    () => () => {
      worker.current?.terminate();
      clearTimeout(timer.current);
      subscription.current?.dispose();
    },
    [],
  );
  function run() {
    stop();
    setRunning(true);
    setResult(null);
    const w = new Worker(new URL("./simulation.worker.js", import.meta.url), {
      type: "module",
    });
    worker.current = w;
    notify(`运行 ${script.name} · 离线 24 h / 步长 5 min`);
    timer.current = setTimeout(() => {
      stop();
      notify("脚本运行超时，已终止（5 秒限制）", "错误");
    }, 5000);
    w.onmessage = ({ data }) => {
      stop();
      if (data.error) notify(data.error, "错误");
      else {
        setResult(data.result);
        data.result.logs.forEach((l) =>
          notify(`${formatTime(l.time)} ${l.message}`),
        );
        notify("离线仿真完成 · 简化模型");
        dock.current?.getPanel("results")?.api.setActive();
      }
    };
    w.onerror = (e) => {
      stop();
      notify(e.message || "脚本执行失败", "错误");
    };
    w.postMessage({ source: script.source, params: project.params });
  }
  function openNode(sel, title) {
    setSelection(sel);
    const api = dock.current;
    if (!api) return;
    const id = nodeId(sel);
    let panel = api.getPanel(id);
    if (!panel)
      panel = api.addPanel({
        id,
        component: "editor",
        title: title || "编辑",
        params: sel,
        position: api.getPanel("editor")
          ? { referencePanel: "editor", direction: "within" }
          : undefined,
      });
    else {
      panel.api.updateParameters(sel);
      if (title) panel.api.setTitle(title);
    }
    panel.api.setActive();
  }
  function openScript(s) {
    setScriptId(s.id);
    const api = dock.current;
    let panel =
      api.getPanel(`script-${s.id}`) ||
      api.panels.find((p) => p.params?.scriptId === s.id);
    if (!panel)
      panel = api.addPanel({
        id: `script-${s.id}`,
        component: "script",
        title: s.name,
        params: { scriptId: s.id },
        position: api.getPanel("editor")
          ? { referencePanel: "editor", direction: "within" }
          : undefined,
      });
    panel.api.setActive();
  }
  function defaultLayout(api) {
    api.clear();
    const strategy = mode === "strategy";
    api.addPanel({
      id: "editor",
      component: strategy ? "script" : "editor",
      title: strategy
        ? script.name
        : template?.name || comm?.name || device?.name || "设备总览",
      params: strategy ? { scriptId: script.id } : selection,
    });
    api.addPanel({
      id: "tree",
      component: "tree",
      title: strategy ? "策略资源管理器" : "设备资源管理器",
      position: { referencePanel: "editor", direction: "left" },
      initialWidth: 240,
    });
    api.addPanel({
      id: "properties",
      component: "properties",
      title: strategy ? "策略参数与绑定" : "属性",
      position: { referencePanel: "editor", direction: "right" },
      initialWidth: 250,
    });
    if (!strategy)
      api.addPanel({
        id: "model",
        component: "model",
        title: "设备视图",
        position: { referencePanel: "properties", direction: "below" },
        initialHeight: 300,
      });
    api.addPanel({
      id: "results",
      component: strategy ? "results" : "logs",
      title: strategy ? "仿真结果" : "事件日志",
      position: { referencePanel: "editor", direction: "below" },
      initialHeight: 245,
    });
    api.addPanel({
      id: "logs",
      component: "logs",
      title: strategy ? "输出日志" : "通信报文",
      params: { packets: !strategy },
      position: { referencePanel: "results", direction: "within" },
      inactive: true,
    });
    api.addPanel({
      id: "errors",
      component: "logs",
      title: strategy ? "错误" : "配置校验",
      params: { errors: true },
      position: { referencePanel: "results", direction: "within" },
      inactive: true,
    });
    api.getPanel("editor").api.setActive();
  }
  function ready({ api }) {
    dock.current = api;
    let restored = false;
    try {
      const layout = localStorage.getItem(`essim-layout-${mode}-v4`);
      if (layout) {
        api.fromJSON(JSON.parse(layout));
        restored = true;
      }
    } catch {}
    if (!restored) defaultLayout(api);
    subscription.current?.dispose();
    const layoutSub = api.onDidLayoutChange(() => {
      try {
        localStorage.setItem(
          `essim-layout-${mode}-v4`,
          JSON.stringify(api.toJSON()),
        );
      } catch {}
    });
    const activeSub = api.onDidActivePanelChange(({ panel }) => {
      if (panel?.params?.focus)
        setSelection({
          focus: panel.params.focus,
          deviceId: panel.params.deviceId || null,
          commId: panel.params.commId || null,
          templateId: panel.params.templateId || null,
        });
      if (panel?.params?.scriptId) setScriptId(panel.params.scriptId);
    });
    subscription.current = {
      dispose() {
        layoutSub.dispose();
        activeSub.dispose();
      },
    };
  }
  useEffect(() => {
    const handle = (e) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        if (saveProject()) notify("项目已保存");
      }
    };
    window.addEventListener("keydown", handle);
    return () => window.removeEventListener("keydown", handle);
  }, [project]);
  function createScript() {
    const id = crypto.randomUUID();
    const next = {
      id,
      name: `strategy_${project.scripts.length + 1}.js`,
      source: defaultScript,
    };
    setProject((p) => ({ ...p, scripts: [...p.scripts, next] }));
    setScriptId(id);
    openScript(next);
    notify("新策略脚本已创建");
  }
  const ctx = {
    runtime,
    setRuntime,
    simulatorSessions, setSimulatorSessions,
    createScript,
    project,
    setProject,
    mode,
    selection,
    setSelection,
    device,
    comm,
    template,
    script,
    scriptId,
    setScriptId,
    models,
    setModels,
    notify,
    logs,
    result,
    running,
    run,
    stop,
    updateDevice,
    updateTemplate,
    updateComm,
    openNode,
    openScript,
    setModal,
    saveProject,
  };
  return (
    <Context.Provider value={ctx}>
      <div className="desktop">
        <div className="menubar">
          <span className="app-brand">
            <LayersIcon /> ESSim
          </span>
          <AppMenu label="文件" items={[
            ...(mode==='device' ? [{label:'新建从站模拟器',run:()=>setModal({role:'从站'})},{label:'新建主站模拟器',run:()=>setModal({role:'主站'})}] : [{label:'新建策略',run:createScript}]),
            {label:'保存项目　Ctrl/Cmd+S',run:()=>{if(saveProject())notify('项目已保存');}}
          ]}/>
          <button onClick={() => notify("在编辑器中使用快捷键进行编辑与撤销")}>
            编辑
          </button>
          <AppMenu label="视图" items={[{label:"恢复布局",run:()=>defaultLayout(dock.current)}]}/>
          <button
            onClick={() =>
              mode === "strategy" ? run() : notify("真实协议引擎尚未接入")
            }
          >
            运行
          </button>
          <button
            onClick={() => notify("拖动面板标题可停靠；双击树节点打开标签页")}
          >
            帮助
          </button>
          <span className="menubar-right">
            储能协议实验室 <span className="prototype">原型</span>
          </span>
        </div>
        <div className="body">
          <nav className="activity">
            <button
              className={mode === "device" ? "active" : ""}
              onClick={() => setMode("device")}
            >
              <Box />
              <span>设备</span>
            </button>
            <button
              className={mode === "strategy" ? "active" : ""}
              onClick={() => setMode("strategy")}
            >
              <Code2 />
              <span>策略</span>
            </button>
          </nav>
          <div className="dock-host">
            <DockviewReact
              key={mode}
              components={components}
              theme={themeLight}
              onReady={ready}
            />
          </div>
        </div>
        <footer>
          <span>
            <span className={running ? "status-dot running" : "status-dot"} />
            {running ? "正在运行离线仿真" : message}
          </span>
          <span>
            {mode === "device" ? "设备配置" : "策略工作台"} · 本地工作区
          </span>
        </footer>
        {modal && <DeviceDialog options={modal === true ? {role:"从站"} : modal} close={() => setModal(false)} />}
      </div>
    </Context.Provider>
  );
}
function LayersIcon() {
  return <Box size={16} className="brand-symbol" />;
}
const resourceTreeStyle = {
  "--trees-padding-inline-override": "4px",
  "--trees-item-padding-x-override": "4px",
  "--trees-item-margin-x-override": "0px",
  "--trees-level-gap-override": "4px",
  "--trees-bg-override": "#f8fafc",
  "--trees-fg-override": "#243140",
  "--trees-accent-override": "#146abf",
  "--trees-selected-bg-override": "#dcedfe",
  "--trees-selected-fg-override": "#146abf",
  "--trees-bg-muted-override": "#eef3f8",
  "--trees-font-size-override": "12px",
  "--trees-font-family-override": "inherit",
  "--trees-focus-ring-color-override": "#146abf",
};
function selectedTreePaths(tree, mode, selection, scriptId) {
  const path =
    mode === "strategy" ? tree.script.get(scriptId) : selectionPath(tree, selection);
  return path ? [path] : [];
}
function renameErrorText(error) {
  const text = String(error || "");
  if (text.includes("empty")) return "名称不能为空";
  if (text.includes("/")) return "名称不能包含 /";
  if (text.includes("already exists")) return "这个位置已有同名节点";
  return "无法重命名";
}
function Tree() {
  const lab = useLab();
  return lab.mode === "strategy" ? <LegacyTree /> : <ProjectTree />;
}
function ProjectTree() {
  const { project, setProject, openNode, setModal, notify, simulatorSessions } = useLab();
  const [menu, setMenu] = useState(null), [edit, setEdit] = useState(null);
  const folders = project.folders || [];
  useEffect(() => { const close = () => setMenu(null); window.addEventListener('click', close); return () => window.removeEventListener('click', close); }, []);
  const context = (e, item = null, kind = 'folder') => { e.preventDefault(); e.stopPropagation(); setMenu({x:Math.min(e.clientX, window.innerWidth-250), y:Math.min(e.clientY, window.innerHeight-300), item, kind}); };
  const patch = (kind, id, values) => setProject(p => ({...p, [kind]: (p[kind] || []).map(i => i.id === id ? {...i,...values} : i)}));
  function createModel(parentId) {
    const model = blankTemplate({name:'新 Modbus 点表模型', folderId:parentId});
    setProject(p => ({...p, templates:[...p.templates,model]}));
    openNode({focus:'template',templateId:model.id}, model.name);
  }
  function download(kind, item) {
    const data = exportProjectFile(project,kind,item.id);
    const url = URL.createObjectURL(new Blob([JSON.stringify(data,null,2)], {type:'application/json'}));
    const a = document.createElement('a'); a.href=url; a.download=`${item.name.replace(/[\\/:*?"<>|]/g,'_')}.${kind === 'template' ? 'modbus-model' : 'modbus-simulator'}.json`; a.click(); setTimeout(() => URL.revokeObjectURL(url),1000);
    notify('配置文件已导出');
  }
  function remove(target) {
    const {item,kind} = target;
    if (kind === 'folder') {
      if (folders.some(f => f.parentId === item.id) || [...project.templates,...project.devices].some(f => f.folderId === item.id)) return notify('请先移动或删除文件夹内的内容','错误');
      setProject(p => ({...p,folders:p.folders.filter(f => f.id !== item.id)}));
    } else if (kind === 'template') {
      if (project.devices.some(d => d.comms.some(c => c.templateId === item.id))) return notify('模型仍被模拟器引用，请先解除引用','错误');
      setProject(p => ({...p,templates:p.templates.filter(f => f.id !== item.id)}));
    } else setProject(p => ({...p,devices:p.devices.filter(f => f.id !== item.id)}));
    notify('已删除，请保存项目');
  }
  const tree = buildProjectFileTree(project);
  const latest = useRef(); latest.current = {tree,openNode,simulatorSessions};
  const syncing = useRef(false);
  const {model} = useFileTree({
    preparedInput:tree.prepared, initialExpansion:'open', flattenEmptyDirectories:false,
    density:'compact', icons:resourceTreeIcons, unsafeCSS:projectTreeCss,
    composition:{contextMenu:{enabled:true,triggerMode:'right-click',buttonVisibility:'when-needed'}},
    onSelectionChange(paths) {
      if (syncing.current) return;
      const action = latest.current.tree.actions.get(canonicalTreePath(paths[0]));
      if (action && action.kind !== 'folder') latest.current.openNode(
        action.kind === 'template' ? {focus:'template',templateId:action.item.id} : {focus:'device',deviceId:action.item.id}, action.item.name);
    },
    renderRowDecoration({item}) {
      const action = latest.current.tree.actions.get(canonicalTreePath(item.path));
      if (action?.kind !== 'device') return null;
      const connections = action.item.comms.map(c => `${c.protocol} · ${c.role}\n${c.role === '从站' ? '监听' : '目标'}：${c.host}:${c.port}\nUnit ID：${c.unitId ?? '—'} · 周期：${c.interval} ms`).join('\n\n');
      return {icon:{name:'essim-wifi-offline',width:16,height:16,viewBox:'0 0 16 16'}, title:`${action.item.comms.some(c=>latest.current.simulatorSessions[c.id])?'本地模拟运行中（不代表真实网络在线）':'未启动通信（原型未接入协议引擎）'}\n\n${connections || '尚未配置通讯'}`};
    }
  });
  useEffect(() => {
    const collapsed = tree.directories.filter(path => model.getItem(path)?.isExpanded() === false);
    syncing.current = true;
    model.resetPaths({preparedInput:tree.prepared});
    for (const path of collapsed) model.getItem(path)?.collapse();
    syncing.current = false;
  },[tree.signature,model]);
  function menuItems(target,close) {
    const parentId = target.kind === 'folder' ? target.item?.id || '' : target.item?.folderId || '';
    const collection = target.kind === 'folder' ? 'folders' : target.kind === 'template' ? 'templates' : 'devices';
    return [
      ['新建文件夹', () => setEdit({action:'create',kind:'folders',name:'新文件夹',parentId})],
      ['新建 Modbus 点表模型',() => createModel(parentId)],
      ['新建 Modbus 从站模拟器',() => setModal({role:'从站',folderId:parentId})],
      ['新建 Modbus 主站模拟器',() => setModal({role:'主站',folderId:parentId})],
      ...(target.item ? [
        ['重命名',() => setEdit({action:'rename',kind:collection,item:target.item,name:target.item.name})],
        ['移动到…',() => setEdit({action:'move',kind:collection,item:target.item,parentId:target.item.parentId || target.item.folderId || ''})],
        ...(target.kind !== 'folder' ? [['导出配置文件',() => download(target.kind,target.item)]] : []),
        ['删除',() => remove(target)]
      ] : [])
    ].map(([label,run]) => <button role="menuitem" key={label} onClick={() => {close();run();}}>{label}</button>);
  }
  return <div className="tree-panel" onContextMenu={e => { if (!e.target.closest?.(".resource-tree")) context(e); }}>
    <div className="tree-tools"><span>项目文件</span><button title="新建文件" onClick={e => {e.stopPropagation();context(e);}}><Plus size={14}/></button></div>
    <FileTree model={model} className="resource-tree" style={resourceTreeStyle}
      renderContextMenu={(item,context) => {
        const target = tree.actions.get(canonicalTreePath(item.path));
        return target ? <div className="tree-menu" role="menu" onContextMenu={e => e.stopPropagation()}>{menuItems(target,() => context.close({restoreFocus:false}))}</div> : null;
      }}/>
    <p className="tree-hint">右键新建文件夹、点表模型或模拟器</p>
    {menu && <div className="tree-menu project-menu" style={{left:menu.x,top:menu.y}} role="menu" onClick={e => e.stopPropagation()}>{menuItems(menu,() => setMenu(null))}</div>}
    {edit && <div className="dialog-backdrop"><form className="dialog" onSubmit={e => {
      e.preventDefault();
      if (edit.action === 'create') setProject(p => ({...p,folders:[...(p.folders || []),{id:crypto.randomUUID(),name:edit.name.trim() || '未命名',parentId:edit.parentId}]}));
      else patch(edit.kind,edit.item.id,edit.action === 'rename' ? {name:edit.name.trim() || '未命名'} : {[edit.kind === 'folders' ? 'parentId' : 'folderId']:edit.parentId});
      setEdit(null);notify('项目结构已更新，请保存项目');
    }}><div className="dialog-title">{edit.action === 'move' ? '移动到文件夹' : edit.action === 'create' ? '新建文件夹' : '重命名'}</div><div className="config-form">
      {edit.action === 'move' ? <label>目标文件夹<select value={edit.parentId} onChange={e => setEdit({...edit,parentId:e.target.value})}><option value="">项目根目录</option>{folders.filter(f => edit.kind !== 'folders' || !folderDescendants(folders,edit.item.id).has(f.id)).map(f => <option key={f.id} value={f.id}>{f.name} · {f.id.slice(0,6)}</option>)}</select></label> : <Field label="名称" value={edit.name} change={name => setEdit({...edit,name})}/>}
      <div className="dialog-actions"><button type="button" onClick={() => setEdit(null)}>取消</button><button className="primary">确定</button></div>
    </div></form></div>}
  </div>;
}
function LegacyTree() {
  const {
    project,
    mode,
    selection,
    openNode,
    openScript,
    scriptId,
    setModal,
    setProject,
    updateDevice,
    notify,
    createScript,
  } = useLab();
  const firstTree = useRef(null);
  if (firstTree.current == null) firstTree.current = buildResourceTree(project, mode);
  const actionsRef = useRef(firstTree.current.actions);
  const signatureRef = useRef(firstTree.current.signature);
  const directoriesRef = useRef(firstTree.current.directories);
  const applyingRef = useRef(false);
  const activateRef = useRef(() => {});
  const modelRef = useRef(null);
  const namingRef = useRef(null);
  const projectRef = useRef(project);
  const modeRef = useRef(mode);
  const openNodeRef = useRef(openNode);
  const notifyRef = useRef(notify);
  const commitTemplateRenameRef = useRef(() => {});
  projectRef.current = project;
  modeRef.current = mode;
  openNodeRef.current = openNode;
  notifyRef.current = notify;
  const { model } = useFileTree({
    preparedInput: firstTree.current.prepared,
    initialExpansion: "open",
    flattenEmptyDirectories: false,
    density: "compact",
    icons: resourceTreeIcons,
    unsafeCSS: resourceTreeCss,
    initialSelectedPaths: selectedTreePaths(
      firstTree.current,
      mode,
      selection,
      scriptId,
    ),
    composition: {
      contextMenu: {
        enabled: true,
        triggerMode: "both",
        buttonVisibility: "when-needed",
      },
    },
    renaming: {
      canRename(item) {
        return (
          actionsRef.current.get(canonicalTreePath(item.path))?.kind === "template"
        );
      },
      onRename(event) {
        commitTemplateRenameRef.current(event);
      },
      onError(error) {
        notifyRef.current(renameErrorText(error), "错误");
      },
    },
    onSelectionChange(paths) {
      if (applyingRef.current || namingRef.current) return;
      const raw = paths[0];
      const action = actionsRef.current.get(canonicalTreePath(raw));
      const openDirectory = Boolean(action?.open && raw?.endsWith("/"));
      // 重命名输入框在这次选择之后才挂上。晚一帧再打开编辑器，避免抢走输入焦点。
      requestAnimationFrame(() => {
        if (namingRef.current) return;
        const input = modelRef.current
          ?.getFileTreeContainer()
          ?.shadowRoot?.querySelector("[data-item-rename-input]");
        if (input) return;
        activateRef.current(action);
        if (!openDirectory) return;
        const item = modelRef.current?.getItem(raw);
        if (item?.isDirectory() && !item.isExpanded()) item.expand();
      });
    },
    renderRowDecoration({ item }) {
      const action = actionsRef.current.get(canonicalTreePath(item.path));
      if (!action?.title) return null;
      return { text: "\u2060", title: action.title };
    },
  });
  modelRef.current = model;
  function activate(action) {
    if (!action?.open) return;
    if (action.open.scriptId) {
      const script = project.scripts.find((item) => item.id === action.open.scriptId);
      if (script) openScript(script);
      return;
    }
    openNode(action.open.sel, action.open.title);
  }
  activateRef.current = activate;
  function syncTree(next) {
    const collapsed = directoriesRef.current.filter((path) => {
      const item = model.getItem(path);
      return item?.isDirectory() && !item.isExpanded();
    });
    applyingRef.current = true;
    model.resetPaths({ preparedInput: next.prepared });
    for (const path of collapsed) {
      const item = model.getItem(path);
      if (item?.isDirectory() && item.isExpanded()) item.collapse();
    }
    signatureRef.current = next.signature;
    directoriesRef.current = next.directories;
    actionsRef.current = next.actions;
    applyingRef.current = false;
  }
  function settleNaming() {
    const naming = namingRef.current;
    if (!naming || naming.phase !== "editing") return;
    naming.stop?.();
    namingRef.current = null;
    if (!naming.openAfter) return;
    const template = projectRef.current.templates.find(
      (item) => item.id === naming.id,
    );
    if (!template) return;
    notifyRef.current("已创建 Modbus 点表");
    openNodeRef.current(
      { focus: "template", templateId: template.id, initialTab: "点表" },
      naming.committedName || template.name,
    );
  }
  function renameInputOf(event) {
    const node = event.composedPath?.()[0];
    return node instanceof HTMLInputElement &&
      node.hasAttribute("data-item-rename-input")
      ? node
      : null;
  }
  function watchNaming(naming) {
    const pending = () => {
      let frames = 0;
      const tick = () => {
        if (namingRef.current !== naming) return;
        const input = model
          .getFileTreeContainer()
          ?.shadowRoot?.querySelector("[data-item-rename-input]");
        if (input) {
          if (frames < 8) {
            frames += 1;
            requestAnimationFrame(tick);
          }
          return;
        }
        settleNaming();
      };
      requestAnimationFrame(tick);
    };
    const onKeyDown = (event) => {
      if (namingRef.current !== naming || !renameInputOf(event)) return;
      if (event.isComposing || event.keyCode === 229) return;
      if (event.key !== "Enter") return;
      naming.openAfter = true;
      pending();
    };
    const onFocusOut = (event) => {
      if (namingRef.current !== naming || !renameInputOf(event)) return;
      pending();
    };
    document.addEventListener("keydown", onKeyDown, true);
    document.addEventListener("focusout", onFocusOut, true);
    naming.stop = () => {
      document.removeEventListener("keydown", onKeyDown, true);
      document.removeEventListener("focusout", onFocusOut, true);
    };
  }
  function commitTemplateRename({ sourcePath, destinationPath }) {
    const action = actionsRef.current.get(canonicalTreePath(sourcePath));
    if (action?.kind !== "template") return;
    const id = action.open?.sel?.templateId;
    if (!id) return;
    const name = sanitizeTreeLabel(visibleTreeLabel(destinationPath));
    const naming = namingRef.current;
    if (naming?.id === id) naming.committedName = name;
    const nextProject = {
      ...projectRef.current,
      templates: projectRef.current.templates.map((item) =>
        item.id === id ? { ...item, name } : item,
      ),
    };
    setProject(nextProject);
    // 库会按输入文本移动路径，零宽类型标记被丢掉。下一拍按模板重建。
    queueMicrotask(() => {
      if (modeRef.current !== "device") return;
      syncTree(buildResourceTree(nextProject, "device"));
    });
  }
  commitTemplateRenameRef.current = commitTemplateRename;
  function addModbusTemplate() {
    if (namingRef.current) return;
    const template = blankTemplate({
      name: "新采集表",
      protocol: "Modbus TCP",
    });
    namingRef.current = {
      id: template.id,
      path: null,
      phase: "pending",
      openAfter: false,
    };
    setProject((current) => ({
      ...current,
      templates: [...current.templates, template],
    }));
  }
  function addComm(d) {
    const comm = blankComm(d.type);
    comm.name =
      d.type === "EMS"
        ? `连接 ${d.comms.length + 1}`
        : `${comm.protocol} 从站`;
    updateDevice(d.id, { comms: [...d.comms, comm] });
    openNode(
      { focus: "comm", deviceId: d.id, commId: comm.id, initialTab: "通信配置" },
      commLabel(comm),
    );
    notify("已创建通讯");
  }
  function selectOnly(path) {
    applyingRef.current = true;
    for (const selected of model.getSelectedPaths()) {
      if (canonicalTreePath(selected) !== path) model.getItem(selected)?.deselect();
    }
    const item = model.getItem(path);
    if (item && !item.isSelected()) item.select();
    model.focusPath(path);
    applyingRef.current = false;
  }
  useEffect(() => {
    const next = buildResourceTree(project, mode);
    actionsRef.current = next.actions;
    if (next.signature !== signatureRef.current) syncTree(next);
    const naming = namingRef.current;
    if (naming?.phase === "pending") {
      const path = next.library.get(naming.id);
      if (path && model.startRenaming(path, { removeIfCanceled: true })) {
        naming.path = path;
        naming.phase = "editing";
        watchNaming(naming);
        return;
      }
      namingRef.current = null;
    }
    if (namingRef.current?.phase === "editing") {
      if (next.library.get(namingRef.current.id)) return;
      namingRef.current = null;
    }
    const [path] = selectedTreePaths(next, mode, selection, scriptId);
    if (!path || !model.getItem(path)) return;
    const selected = model.getSelectedPaths().map(canonicalTreePath);
    if (selected.length === 1 && selected[0] === path) return;
    selectOnly(path);
  }, [project, mode, selection, scriptId, model]);
  useEffect(() => {
    return model.onMutation("remove", (event) => {
      const naming = namingRef.current;
      if (!naming?.path) return;
      if (canonicalTreePath(event.path) !== canonicalTreePath(naming.path)) return;
      const id = naming.id;
      naming.stop?.();
      namingRef.current = null;
      setProject((current) => ({
        ...current,
        templates: current.templates.filter((item) => item.id !== id),
      }));
    });
  }, [model]);
  function menuAction(item) {
    const action = actionsRef.current.get(canonicalTreePath(item.path));
    if (!action) return null;
    if (action.kind === "library")
      return { label: "新建 Modbus 点表", run: addModbusTemplate };
    if (action.kind === "devices")
      return { label: "新建设备", run: () => setModal(true) };
    if (action.kind === "device")
      return {
        label: "添加通讯",
        run: () => {
          const device = project.devices.find((item) => item.id === action.deviceId);
          if (device) addComm(device);
        },
      };
    if (action.open) return { label: "打开", run: () => activate(action) };
    return null;
  }
  return (
    <div className="tree-panel">
      <div className="tree-tools">
        <span>{mode === "device" ? "模拟设备" : "储能策略"}</span>
        <span className="tree-tool-actions">
          {mode === "device" && (
            <button
              type="button"
              className="modbus-create"
              title="新建 Modbus 点表"
              aria-label="新建 Modbus 点表"
              onClick={addModbusTemplate}
              dangerouslySetInnerHTML={{ __html: modbusBadgeSvg }}
            />
          )}
          <button
            title={mode === "device" ? "新建设备" : "新建策略"}
            onClick={() => (mode === "device" ? setModal(true) : createScript())}
          >
            <Plus size={14} />
          </button>
        </span>
      </div>
      <FileTree
        model={model}
        className="resource-tree"
        style={resourceTreeStyle}
        renderContextMenu={(item, context) => {
          const command = menuAction(item);
          if (!command) return null;
          return (
            <div className="tree-menu" role="menu">
              <button
                type="button"
                role="menuitem"
                onClick={() => {
                  context.close({ restoreFocus: false });
                  command.run();
                }}
              >
                {command.label}
              </button>
            </div>
          );
        }}
      />
      {mode === "strategy" && (
        <p className="tree-hint">
          脚本修改后可运行离线仿真。
          <br />
          JavaScript · on_tick(ctx)
        </p>
      )}
    </div>
  );
}
function Editor({ params, api }) {
  const { project, selection, setSelection, updateDevice, openNode } = useLab();
  const focus = params.focus || "device";
  const device = project.devices.find((d) => d.id === params.deviceId);
  const comm = device?.comms.find((c) => c.id === params.commId);
  const template = project.templates.find(
    (t) => t.id === (focus === "template" ? params.templateId : undefined),
  );
  const [tab, setTab] = useState(params.initialTab || "总览");
  useEffect(() => {
    if (params.initialTab) setTab(params.initialTab);
  }, [params.initialTab, params.focus, params.templateId, params.commId]);
  useEffect(() => {
    const title =
      focus === "template"
        ? template?.name
        : focus === "comm"
          ? comm && commLabel(comm)
          : device?.name;
    if (title) api.setTitle(title);
  }, [focus, template?.name, comm?.name, comm?.host, comm?.port, device?.name]);
  useEffect(() => {
    setSelection({
      focus,
      deviceId: params.deviceId || null,
      commId: params.commId || null,
      templateId: params.templateId || null,
    });
  }, [focus, params.deviceId, params.commId, params.templateId]);
  const missing =
    (focus === "device" && !device) ||
    (focus === "comm" && !comm) ||
    (focus === "template" && !template);
  if (missing)
    return <div className="empty">对象已不存在，请从资源树重新选择。</div>;
  if (focus !== 'template') {
    const selectedComm = comm || device.comms.find(c => c.id === selection.commId) || device.comms[0];
    return <div className="editor-panel"><SimulatorSheet key={`${selectedComm?.id || device.id}:${selectedComm?.templateId || ""}:${JSON.stringify(project.templates.find(t=>t.id===selectedComm?.templateId)?.points || [])}`} device={device} comm={selectedComm}/></div>;
  }
  const tabs =
    focus === "template"
      ? []
      : focus === "comm"
        ? comm.role === "主站"
          ? ["总览", "通信配置", "接收与发送"]
          : ["总览", "通信配置"]
        : device.type === "EMS"
          ? ["总览", "设备配置"]
          : ["总览", "设备配置", "置数与控制"];
  const safeTab = tabs.includes(tab) ? tab : tabs[0] || "总览";
  return (
    <div className="editor-panel">
      {tabs.length > 0 && (
        <div className="subtabs">
          {tabs.map((t) => (
            <button
              key={t}
              className={t === safeTab ? "active" : ""}
              onClick={() => setTab(t)}
            >
              {t}
            </button>
          ))}
        </div>
      )}
      <div className="editor-content">
        {focus === "template" ? (
          <Points key={template.id} template={template} />
        ) : focus === "comm" ? (
          safeTab === "接收与发送" ? (
            <RuntimePanel key={comm.id} device={device} comm={comm} owner="comm" />
          ) : safeTab === "通信配置" ? (
            <CommSettings key={comm.id} device={device} comm={comm} />
          ) : (
            <CommOverview device={device} comm={comm} />
          )
        ) : safeTab === "置数与控制" ? (
          <RuntimePanel key={device.id} device={device} owner="device" />
        ) : safeTab === "设备配置" ? (
          <DeviceSettings device={device} />
        ) : (
          <DeviceOverview
            device={device}
            addComm={() => {
              const next = blankComm(device.type);
              next.name =
                device.type === "EMS"
                  ? `连接 ${device.comms.length + 1}`
                  : `${next.protocol} 从站`;
              updateDevice(device.id, { comms: [...device.comms, next] });
              openNode(
                {
                  focus: "comm",
                  deviceId: device.id,
                  commId: next.id,
                  initialTab: "通信配置",
                },
                commLabel(next),
              );
            }}
          />
        )}
      </div>
    </div>
  );
}
function DeviceOverview({ device, addComm }) {
  const { project, openNode } = useLab();
  return (
    <>
      <Section title="设备总览" />
      <PropertyRows
        rows={[
          ["设备名称", device.name],
          ["类型", device.customType || device.type],
          ["说明", device.description || "—"],
          ["通讯", device.comms.length],
        ]}
      />
      <div className="section-row">
        <strong>通讯</strong>
        <button onClick={addComm}>
          <Plus size={14} /> 添加通讯
        </button>
      </div>
      <table className="data-table">
        <thead>
          <tr>
            <th>名称</th>
            <th>角色</th>
            <th>地址</th>
            <th>采集表</th>
          </tr>
        </thead>
        <tbody>
          {device.comms.map((c) => {
            const bound = project.templates.find((t) => t.id === c.templateId);
            return (
              <tr
                key={c.id}
                onDoubleClick={() =>
                  openNode(
                    { focus: "comm", deviceId: device.id, commId: c.id },
                    commLabel(c),
                  )
                }
              >
                <td>
                  <button
                    className="link"
                    onClick={() =>
                      openNode(
                        { focus: "comm", deviceId: device.id, commId: c.id },
                        commLabel(c),
                      )
                    }
                  >
                    {c.name}
                  </button>
                </td>
                <td>{c.role}</td>
                <td>{endpointLabel(c)}</td>
                <td>{bound?.name || "未绑定"}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
      <p className="note">
        {device.type === "EMS"
          ? "EMS 的收发在每条通讯中操作。示例地址不会发起真实连接。"
          : "运行值在「置数与控制」中按设备保存。示例地址不会建立真实监听。"}
      </p>
    </>
  );
}
function CommOverview({ device, comm }) {
  const { project, openNode } = useLab();
  const bound = project.templates.find((t) => t.id === comm.templateId);
  const peers = peersFor(project, comm);
  return (
    <>
      <Section title="通讯总览" />
      <PropertyRows
        rows={[
          ["名称", comm.name],
          ["设备", device.name],
          ["协议", comm.protocol],
          ["角色", comm.role],
          ["地址", endpointLabel(comm)],
          ["采集表", bound?.name || "未绑定"],
          ["点数", bound?.points.length ?? 0],
        ]}
      />
      {bound && (
        <p className="note">
          <button
            className="link"
            onClick={() =>
              openNode(
                {
                  focus: "template",
                  templateId: bound.id,
                  deviceId: device.id,
                  commId: comm.id,
                  initialTab: "点表",
                },
                bound.name,
              )
            }
          >
            打开采集表
          </button>
        </p>
      )}
      <div className="section-row">
        <strong>{comm.role === "从站" ? "指向此监听地址的主站" : "同地址从站"}</strong>
      </div>
      {peers.length ? (
        <ul className="peer-list">
          {peers.map((p) => (
            <li key={p.comm.id}>
              {p.device.name} / {p.comm.name}
            </li>
          ))}
        </ul>
      ) : (
        <p className="note">本工程中没有地址和协议都相同的对端。</p>
      )}
      <p className="note">配置草稿 · 当前未建立真实连接。</p>
    </>
  );
}
function Section({ title }) {
  return (
    <div className="section-row">
      <strong>{title}</strong>
      <span className="muted">配置草稿</span>
    </div>
  );
}
function PropertyRows({ rows }) {
  return (
    <table className="properties-table">
      <tbody>
        {rows.map(([k, v], i) => (
          <tr key={`${k}-${i}`}>
            <th>{k}</th>
            <td>{v}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
function DeviceSettings({ device }) {
  const { updateDevice, notify } = useLab();
  const [draft, setDraft] = useState(device);
  return (
    <form
      className="config-form"
      onSubmit={(e) => {
        e.preventDefault();
        const err = validateDevice(draft);
        if (err) notify(err, "错误");
        else {
          updateDevice(device.id, draft);
          notify("设备配置已更新，请保存项目");
        }
      }}
    >
      <Section title="设备配置" />
      <Field
        label="设备名称"
        value={draft.name}
        change={(v) => setDraft((d) => ({ ...d, name: v }))}
      />
      <label>
        设备类型
        <select
          value={draft.type}
          onChange={(e) => setDraft((d) => ({ ...d, type: e.target.value }))}
        >
          {types.map((t) => (
            <option key={t}>{t}</option>
          ))}
        </select>
      </label>
      {draft.type === "自定义" && (
        <Field
          label="自定义类型"
          value={draft.customType || ""}
          change={(v) => setDraft((d) => ({ ...d, customType: v }))}
        />
      )}
      <Field
        label="设备说明"
        value={draft.description || ""}
        change={(v) => setDraft((d) => ({ ...d, description: v }))}
      />
      <button className="primary">
        <Save size={14} /> 保存设备配置
      </button>
    </form>
  );
}
function Field({ label, value, change, type = "text" }) {
  return (
    <label>
      {label}
      <input
        type={type}
        value={value ?? ""}
        onChange={(e) =>
          change(type === "number" ? e.target.valueAsNumber : e.target.value)
        }
      />
    </label>
  );
}
function CommSettings({ device, comm }) {
  const { project, setProject, updateComm, notify } = useLab();
  const [draft, setDraft] = useState(comm);
  const patch = (k, v) => setDraft((g) => ({ ...g, [k]: v }));
  const choices = project.templates.filter((t) => t.protocol === draft.protocol);
  const selected = project.templates.find((t) => t.id === draft.templateId);
  return (
    <form
      className="config-form"
      onSubmit={(e) => {
        e.preventDefault();
        const err = validateComm(draft, selected);
        if (err) notify(err, "错误");
        else {
          updateComm(device.id, comm.id, draft);
          notify("通讯配置已更新，请保存项目");
        }
      }}
    >
      <Section title="通信配置" />
      <Field label="通讯名称" value={draft.name} change={(v) => patch("name", v)} />
      <label>
        协议
        <select
          value={draft.protocol}
          onChange={(e) => {
            const protocol = e.target.value;
            setDraft((g) => ({
              ...g,
              protocol,
              templateId:
                project.templates.find((t) => t.id === g.templateId)?.protocol ===
                protocol
                  ? g.templateId
                  : "",
            }));
          }}
        >
          {protocols.map((p) => (
            <option key={p}>{p}</option>
          ))}
        </select>
      </label>
      <label>
        通信角色
        <span>{draft.role}（由创建入口确定）</span>
      </label>
      <Field
        label={draft.role === "从站" ? "监听 IPv4 地址" : "目标 IPv4 地址"}
        value={draft.host}
        change={(v) => patch("host", v)}
      />
      <Field
        label="TCP 端口"
        type="number"
        value={draft.port}
        change={(v) => patch("port", v)}
      />
      <Field
        label="采集 / 更新周期 (ms)"
        type="number"
        value={draft.interval}
        change={(v) => patch("interval", v)}
      />
      {draft.protocol === "Modbus TCP" ? (
        <Field
          label="Unit ID"
          type="number"
          value={draft.unitId}
          change={(v) => patch("unitId", v)}
        />
      ) : draft.protocol === "IEC 60870-5-104" ? (
        <Field
          label="公共地址"
          type="number"
          value={draft.commonAddress}
          change={(v) => patch("commonAddress", v)}
        />
      ) : (
        <Field
          label="IED 名称"
          value={draft.iedName}
          change={(v) => patch("iedName", v)}
        />
      )}
      <label>
        采集表
        <select
          aria-label="引用采集表"
          value={choices.some((t) => t.id === draft.templateId) ? draft.templateId : ""}
          onChange={(e) => patch("templateId", e.target.value)}
        >
          <option value="">未绑定</option>
          {choices.map((t) => (
            <option key={t.id} value={t.id}>
              {t.name}
            </option>
          ))}
        </select>
      </label>
      <button
        type="button"
        onClick={() => {
          const template = blankTemplate({
            name: `${device.customType || device.type} ${draft.protocol} 标准表`,
            protocol: draft.protocol,
            deviceType: device.type,
            points: demoPoints(device.type),
          });
          setProject((p) => ({ ...p, templates: [...p.templates, template] }));
          patch("templateId", template.id);
          notify("已新建采集表，保存通讯后完成引用");
        }}
      >
        <Plus size={14} /> 按当前协议新建采集表
      </button>
      <button className="primary">
        <Save size={14} /> 保存配置
      </button>
      <p className="note">
        从站地址是本机监听地址，主站地址是要连接的目标。原型只保存配置，不会监听端口或连接外部设备。
      </p>
    </form>
  );
}
function downloadConfig(data, name) {
  const url = URL.createObjectURL(new Blob([JSON.stringify(data,null,2)],{type:'application/json'}));
  const a = document.createElement('a'); a.href=url; a.download=`${name.replace(/[\\/:*?"<>|]/g,'_')}.json`; a.click(); setTimeout(()=>URL.revokeObjectURL(url),1000);
}
function Tool({label,disabled,onClick,children}) {
  return <button type="button" title={label} aria-label={label} disabled={disabled} onClick={onClick}>{children}</button>;
}
function SimulatorSheet({device,comm}) {
  const {project,runtime,setRuntime,simulatorSessions,setSimulatorSessions,notify} = useLab();
  const grid=useRef(); const [active,setActive]=useState('遥测'), [batch,setBatch]=useState(null);
  const points=comm ? templatePoints(project,comm.templateId) : [];
  const [sheets] = useState(()=>Object.fromEntries(categories.map(category=>[category,new InMemoryDataSource({schema:{fields:pointFields.filter(f=>f.id!=='scale').map(f=>({...f,width:({name:130,address:85,type:85,unit:50,value:85,access:55})[f.id],...(f.id==='value'?{name:'当前值'}:{})}))},rows:points.filter(p=>pointCategory(p)===category).map(p=>({...p,value:runtime[runtimeKey(device,comm,p)]?.value??p.value}))})])));
  const master=comm?.role==='主站',started=!!simulatorSessions[comm?.id];
  useEffect(()=>{
    if (!comm) return;
    for (const category of categories) {
      const data=sheets[category];
      data.getRows(0,data.getRowCount()-1).forEach((row,i)=>{
        const value=runtime[runtimeKey(device,comm,row)]?.value??points.find(p=>p.id===row.id)?.value;
        if (value!==undefined && data.getCell(i,'value')!==value) data.updateCell(i,'value',value);
      });
    }
  },[runtime]);
  if (!comm || !comm.templateId) return <div className="empty">请在右侧属性栏配置通讯并选择点表模型。</div>;
  const data=sheets[active];
  function selectedRows() {
    const selection=grid.current?.grid.getSelection();const range=selection?.selectedRange;
    const start=range?.startRow??selection?.activeCell?.rowIndex;
    if (start==null) return [];
    return data.getRows(start,range?.endRow??start);
  }
  function apply(operation,override) {
    try {
      const rows=selectedRows().map(row=>override===undefined?row:{...row,value:override});
      if ((operation==='read'||operation==='write')&&!started) throw Error('请先连接本地模拟对端');
      const peers=peersFor(project,comm); if ((operation==='read'||operation==='write')&&peers.length!==1) throw Error('本地目标须唯一匹配，请检查目标地址');
      const peer=peers[0];
      const updates=prepareRuntimeBatch({device,comm,points,rows,operation,runtime,peer,peerPoints:peer?templatePoints(project,peer.comm.templateId):[]});
      setRuntime(r=>({...r,...updates}));setBatch(null);notify(`${rows.length} 个点已${operation==='write'?'写入本地对端':operation==='read'?'读取':operation==='reset'?'恢复初值':'置数'}（本地模拟）`);
    } catch(error) {notify(error.message,'错误');}
  }
  function toggle() {
    if (!started && master && peersFor(project,comm).length!==1) return notify('目标地址须唯一匹配本地从站，原型不会连接外部设备','错误');
    setSimulatorSessions(state=>({...state,[comm.id]:!started}));notify(started?'本地模拟已停止':master?'已连接本地模拟对端':'从站本地模拟已启动');
  }
  return <div className="points simulator-sheet">
    <div className="panel-toolbar compact-toolbar">
      <Tool label={master?(started?'断开（本地模拟）':'连接（本地模拟）'):(started?'停止（本地模拟）':'启动（本地模拟）')} onClick={toggle}>{master?(started?<Unplug/>:<Plug/>):(started?<Square/>:<Play/>)}</Tool><i/>
      {master ? <><Tool label="读取选区" disabled={!started||!isTelemetry(active)} onClick={()=>apply('read')}><ArrowDownToLine/></Tool><Tool label="写入选区（明确发送控制）" disabled={!started||isTelemetry(active)} onClick={()=>apply('write')}><ArrowUpFromLine/></Tool></> : <><Tool label="批量置数（选区）" onClick={()=>{if(!selectedRows().length)return notify('请先选择数据区域','错误');setBatch('');}}><Pencil/></Tool><Tool label="恢复选区初始值" onClick={()=>apply('reset')}><RefreshCw/></Tool></>}
      <i/><Tool label="导出模拟器配置及模型" onClick={()=>downloadConfig(exportProjectFile(project,'device',device.id),device.name+'.modbus-simulator')}><Download/></Tool>
    </div>
    {batch!==null && <form className="batch-input" onSubmit={e=>{e.preventDefault();apply('set',batch);}}><label>选区统一值 <input autoFocus type="number" value={batch} onChange={e=>setBatch(e.target.value)}/></label><button>应用</button><button type="button" onClick={()=>setBatch(null)}>取消</button></form>}
    <NovaSheetGrid excelHeaders rowHeaderCornerLabel="序号" key={active} ref={grid} data={data} theme={numberedGridTheme} className="sheet" isCellEditable={({fieldId,rowIndex})=>fieldId==='value'&&(!master||(!isTelemetry(active)&&data.getRows(rowIndex,rowIndex)[0]?.access==='读写'))}/>
    <div className="sheet-tabs" role="tablist" aria-label="四遥运行点表">{categories.map(c=><button key={c} role="tab" aria-selected={active===c} className={active===c?'active':''} onClick={()=>setActive(c)}>{c}</button>)}<span>本地模拟 · 选区操作 · 运行值草稿</span></div>
  </div>;
}
function Points({ template }) {
  const { project, updateTemplate, notify } = useLab();
  const grid = useRef(), importFile=useRef();
  const [active, setActive] = useState("遥测");
  const [pending, setPending] = useState(null);
  const names = templateDeviceNames(project, template.id);
  const [sheets, setSheets] = useState(() =>
    Object.fromEntries(
      categories.map((c) => [
        c,
        {
          revision: 0,
          data: (() => {
            const data = new InMemoryDataSource({
              schema: { fields: pointFields },
              rows: template.points
                .filter((p) => pointCategory(p) === c)
                .map((p) => ({ ...p, category: c })),
            });
            ensureSpareRows(data);
            return data;
          })(),
        },
      ]),
    ),
  );
  const sheet = sheets[active];
  useEffect(() => {
    const stops = categories.map((c) => {
      const data = sheets[c].data;
      const sync = () => ensureSpareRows(data);
      sync();
      return data.subscribe(sync);
    });
    return () => {
      for (const stop of stops) stop();
    };
  }, [sheets]);
  function collectedPoints() {
    return categories.flatMap((c) =>
      sheets[c].data
        .getRows(0, sheets[c].data.getRowCount() - 1)
        .filter((p) => !isBlankPointRow(p))
        .map((p) => ({ ...p, id: p.id || crypto.randomUUID(), category: c })),
    );
  }
  function commit(points) {
    updateTemplate(template.id, { points });
    setPending(null);
    notify(
      names.length
        ? `采集表结构已更新，影响 ${names.join("、")}。请保存项目`
        : "采集表结构已更新，尚未被通讯引用。请保存项目",
    );
  }
  function save() {
    const points = collectedPoints();
    if (names.length > 1) {
      setPending(points);
      return;
    }
    commit(points);
  }
  return (
    <div className="points">
      {pending && (
        <div className="template-banner confirm">
          保存将同时修改 {names.join("、")} 的采集表结构。
          <button className="primary" onClick={() => commit(pending)}>
            确认保存
          </button>
          <button onClick={() => setPending(null)}>取消</button>
        </div>
      )}
      <div className="panel-toolbar compact-toolbar">
        <Tool label="保存点表模型" onClick={save}><Save/></Tool><i/>
        <Tool label="撤销（Ctrl/Cmd+Z）" onClick={()=>grid.current?.grid.undo()}><Undo2/></Tool>
        <Tool label="重做（Ctrl/Cmd+Shift+Z）" onClick={()=>grid.current?.grid.redo()}><Redo2/></Tool><i/>
        <Tool label="新增点（选中行前）" onClick={()=>{const row=grid.current?.grid.getSelection()?.activeCell?.rowIndex??0;grid.current?.grid.insertRows(row,1);}}><ListPlus/></Tool>
        <Tool label="删除选中点" onClick={()=>{const selection=grid.current?.grid.getSelection(),range=selection?.selectedRange;const start=range?.startRow??selection?.activeCell?.rowIndex;if(start==null)return notify('请先选择数据行','错误');grid.current?.grid.deleteRows(Array.from({length:(range?.endRow??start)-start+1},(_,i)=>start+i));}}><Trash2/></Tool><i/>
        <Tool label="导入点表模型 JSON（替换编辑草稿）" onClick={()=>importFile.current?.click()}><Upload/></Tool>
        <Tool label="导出当前点表模型草稿" onClick={()=>downloadConfig({format:'essim.modbus-model',version:1,model:{...template,points:collectedPoints()}},template.name+'.modbus-model')}><Download/></Tool>
        <input ref={importFile} hidden type="file" accept=".json" onChange={async e=>{const file=e.target.files[0];e.target.value='';if(!file)return;try{const imported=JSON.parse(await file.text());if(!['essim.modbus-model','esslab.modbus-model'].includes(imported.format)||imported.version!==1||!Array.isArray(imported.model?.points))throw Error('请选择版本 1 的 ESSim 点表模型文件');const items=imported.model.points;if(items.some(p=>!p.name||!Number.isFinite(Number(p.value))))throw Error('导入点名称或初始值无效');setSheets(Object.fromEntries(categories.map(c=>{const data=new InMemoryDataSource({schema:{fields:pointFields},rows:items.filter(p=>pointCategory(p)===c).map(p=>({...p,id:crypto.randomUUID(),category:c}))});ensureSpareRows(data);return[c,{revision:sheets[c].revision+1,data}];})));notify('模型已导入编辑草稿，请检查后保存');}catch(error){notify(error.message,'错误');}}}/>
      </div>
      <NovaSheetGrid
        excelHeaders
        rowHeaderCornerLabel="序号"
        key={`${active}-${sheet.revision}`}
        ref={grid}
        data={sheet.data}
        theme={numberedGridTheme}
        className="sheet"
      />
      <div className="sheet-tabs" role="tablist" aria-label="四遥采集表">
        {categories.map((c) => (
          <button
            key={c}
            role="tab"
            aria-selected={active === c}
            className={active === c ? "active" : ""}
            onClick={() => setActive(c)}
          >
            {c}
          </button>
        ))}
        <span>{active} · 配置初始值，运行值请到置数与控制修改</span>
      </div>
      <div className="editor-foot">
        四遥切换保留编辑草稿 · 保存全部四遥 · 拖拽填充 · 在行号上右键插入或删除行
      </div>
    </div>
  );
}
function valueKey(kind, id, pointId) {
  return `${kind}:${id}:${pointId}`;
}
function RuntimePanel({ device, comm: fixedComm, owner }) {
  const { project, runtime, setRuntime, notify } = useLab();
  const [commId, setCommId] = useState(fixedComm?.id || device.comms[0]?.id || "");
  const [active, setActive] = useState("遥测");
  const [inputs, setInputs] = useState({});
  const [peerId, setPeerId] = useState("");
  const comm =
    owner === "comm" ? fixedComm : device.comms.find((c) => c.id === commId);
  if (!comm) return <div className="empty">请先为设备创建通讯。</div>;
  if (!comm.templateId)
    return <div className="empty">请先在通信配置中绑定采集表。</div>;
  const points = templatePoints(project, comm.templateId);
  const peers = peersFor(project, comm);
  const peer = peers.find((p) => p.comm.id === peerId);
  const role = owner === "device" ? "从站" : comm.role;
  const telemetry = isTelemetry(active);
  const action =
    role === "从站" ? (telemetry ? "置数" : "接收") : telemetry ? "接收" : "发送";
  const rows = points.filter((p) => pointCategory(p) === active);
  const mine = (pointId) =>
    valueKey(
      owner === "device" ? "dev" : "comm",
      owner === "device" ? device.id : comm.id,
      pointId,
    );
  const theirs = (pointId) =>
    valueKey(
      peer.comm.role === "主站" ? "comm" : "dev",
      peer.comm.role === "主站" ? peer.comm.id : peer.device.id,
      pointId,
    );
  function apply(point) {
    const current = runtime[mine(point.id)];
    const raw = inputs[point.id] ?? String(current?.value ?? point.value);
    if (raw === "") {
      notify("请输入数值", "错误");
      return;
    }
    const value = Number(raw);
    const err = validateAction(role, active, action, value);
    if (err) {
      notify(err, "错误");
      return;
    }
    const now = new Date().toLocaleTimeString("zh-CN", { hour12: false });
    const updates = {};
    let incoming = value;
    try {
      if (action === "发送") {
        if (!peer) throw Error("目标地址上没有同协议的从站");
        const target = findPeerPoint(
          point,
          templatePoints(project, peer.comm.templateId),
        );
        updates[theirs(target.id)] = {
          value,
          time: now,
          status: "已接收控制（本地模拟）",
        };
      }
      if (action === "接收" && peer) {
        const source = findPeerPoint(
          point,
          templatePoints(project, peer.comm.templateId),
        );
        incoming = runtime[theirs(source.id)]?.value ?? source.value;
        const check = validateAction(role, active, action, Number(incoming));
        if (check) throw Error(check);
      }
      if (action === "置数" && peer) {
        const target = findPeerPoint(
          point,
          templatePoints(project, peer.comm.templateId),
        );
        updates[theirs(target.id)] = {
          value,
          time: now,
          status: "已接收数据（本地模拟）",
        };
      }
      updates[mine(point.id)] = {
        value: Number(incoming),
        time: now,
        status:
          action === "发送"
            ? "对端已接收（本地模拟）"
            : action === "接收"
              ? peer
                ? "已接收（本地模拟）"
                : "注入接收（本地模拟）"
              : peer
                ? "已置数并同步对端"
                : "已置数（本地）",
      };
      setRuntime((s) => ({ ...s, ...updates }));
      notify(
        `${role} ${active} ${point.name} ${action}：${incoming} · 本地模拟${peer ? " / " + peer.device.name : ""}`,
      );
    } catch (e) {
      notify(e.message, "错误");
    }
  }
  return (
    <div className="runtime-panel">
      <div className="panel-toolbar">
        {owner === "device" ? (
          <label>
            通讯{" "}
            <select
              aria-label="置数通讯"
              value={comm.id}
              onChange={(e) => {
                setCommId(e.target.value);
                setPeerId("");
              }}
            >
              {device.comms.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </select>
          </label>
        ) : (
          <strong>{comm.name}</strong>
        )}
        <strong className="role-tag">{role}</strong>
        <span className="muted">本地模拟 · 未接入真实通信</span>
      </div>
      <div className="runtime-peer">
        <label>
          本地对端{" "}
          <select
            aria-label="本地对端"
            value={peerId}
            onChange={(e) => setPeerId(e.target.value)}
          >
            <option value="">不连接对端（手动注入）</option>
            {peers.map((p) => (
              <option key={p.comm.id} value={p.comm.id}>
                {p.device.name} / {p.comm.name} · {p.comm.role}
              </option>
            ))}
          </select>
        </label>
        <span>匹配同四遥 + 同地址；遥控接受不代表设备动作完成</span>
      </div>
      <div className="runtime-description">
        {role === "从站"
          ? "从站：遥测 / 遥信置数，接收遥控 / 遥调。值保存在这台设备上。"
          : "主站：接收遥测 / 遥信，发送遥控 / 遥调。值保存在这条连接上。"}
      </div>
      <div className="runtime-table">
        <table className="data-table">
          <thead>
            <tr>
              <th>点名称</th>
              <th>地址</th>
              <th>当前值</th>
              <th>单位</th>
              <th>{action === "接收" && peer ? "对端读取" : "输入值"}</th>
              <th>操作</th>
              <th>状态</th>
              <th>更新时间</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((p) => {
              const state = runtime[mine(p.id)];
              return (
                <tr key={p.id}>
                  <td>{p.name}</td>
                  <td>{p.address}</td>
                  <td>{state?.value ?? p.value}</td>
                  <td>{p.unit || "—"}</td>
                  <td>
                    <input
                      aria-label={`${p.name}输入值`}
                      type="number"
                      disabled={action === "接收" && !!peer}
                      value={inputs[p.id] ?? String(state?.value ?? p.value)}
                      onChange={(e) =>
                        setInputs((i) => ({ ...i, [p.id]: e.target.value }))
                      }
                    />
                  </td>
                  <td>
                    <button className="primary" onClick={() => apply(p)}>
                      {action === "接收"
                        ? peer
                          ? "模拟接收"
                          : "注入接收"
                        : action === "发送"
                          ? "发送" + active
                          : "置数"}
                    </button>
                  </td>
                  <td>{state?.status || "未操作"}</td>
                  <td>{state?.time || "—"}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
        {!rows.length && (
          <div className="empty">
            没有{active}点，请在采集表底部对应页签添加并保存。
          </div>
        )}
      </div>
      <div className="sheet-tabs" role="tablist" aria-label="四遥运行值">
        {categories.map((c) => (
          <button
            role="tab"
            key={c}
            aria-selected={active === c}
            className={active === c ? "active" : ""}
            onClick={() => setActive(c)}
          >
            {c}
          </button>
        ))}
        <span>运行值仅保留本次会话，不修改点表初始值</span>
      </div>
    </div>
  );
}
function CompactProperties() {
  const {project,device,comm,template,selection,updateDevice,updateTemplate,updateComm,setSelection,openNode,notify,simulatorSessions}=useLab();
  const selected=comm||device?.comms.find(c=>c.id===selection.commId)||device?.comms[0];
  const [draft,setDraft]=useState(null);
  useEffect(()=>setDraft(selected?{...selected}:null),[selected]);
  const row=(label,content)=><label className="compact-prop"><span>{label}</span>{content}</label>;
  if(selection.focus==='template'&&template)return <div className="compact-properties"><div className="property-heading">▾ 模型</div>{row('名称',<input value={template.name} onChange={e=>updateTemplate(template.id,{name:e.target.value})}/>)}<PropertyRows rows={[["协议",template.protocol],["点数",template.points.length],["引用",templateDeviceNames(project,template.id).join('、')||'无']]}/></div>;
  if(!device)return <div className="empty">请选择项目文件。</div>;
  const field=(label,key,type='text')=>row(label,<input type={type} value={draft?.[key]??''} onChange={e=>setDraft(d=>({...d,[key]:type==='number'?Number(e.target.value):e.target.value}))}/>);
  return <div className="compact-properties"><div className="property-heading">▾ 基本</div>{row('名称',<input value={device.name} onChange={e=>updateDevice(device.id,{name:e.target.value})}/>)}
    <PropertyRows rows={[["角色",selected?.role||'未配置'],["状态",simulatorSessions[selected?.id]?'本地模拟中':'未启动']]}/>
    {device.comms.length>1&&row('通讯',<select value={selected?.id} onChange={e=>setSelection(s=>({...s,commId:e.target.value}))}>{device.comms.map(c=><option key={c.id} value={c.id}>{c.name}</option>)}</select>)}
    {draft&&<><div className="property-heading">▾ 通讯</div>{row('协议',<span>{draft.protocol}</span>)}{field(draft.role==='从站'?'绑定地址':'目标地址','host')}{field('端口','port','number')}{field('Unit ID','unitId','number')}
    {row('点表模型',<select value={draft.templateId} onChange={e=>setDraft(d=>({...d,templateId:e.target.value}))}><option value="">未绑定</option>{project.templates.filter(t=>t.protocol===draft.protocol).map(t=><option key={t.id} value={t.id}>{t.name}</option>)}</select>)}
    <div className="compact-prop-actions"><button disabled={!draft.templateId} onClick={()=>openNode({focus:'template',templateId:draft.templateId},project.templates.find(t=>t.id===draft.templateId)?.name)}>打开模型</button><button onClick={()=>{const error=validateComm(draft,project.templates.find(t=>t.id===draft.templateId));if(error)return notify(error,'错误');if(!Number.isInteger(Number(draft.unitId))||draft.unitId<1||draft.unitId>247)return notify('Unit ID 应为 1–247','错误');updateComm(device.id,selected.id,draft);notify('配置已应用（本地原型），请保存项目');}}>应用配置</button></div><details><summary>高级</summary>{field('周期 ms','interval','number')}</details></>}
    <p className="compact-note">本地模拟，未接入真实协议引擎。</p>
  </div>;
}
function Properties() {
  const lab=useLab();
  return lab.mode === "device" ? <CompactProperties/> : <LegacyProperties/>;
}
function LegacyProperties() {
  const { mode, selection, device, comm, template, project, setProject } =
    useLab();
  const bound =
    template || project.templates.find((t) => t.id === comm?.templateId);
  const rows =
    selection.focus === "template" && template
      ? [
          ["采集表", template.name],
          ["协议", template.protocol],
          ["适用类型", template.deviceType],
          ["点数", template.points.length],
          ["引用", templateDeviceNames(project, template.id).join("、") || "无"],
        ]
      : selection.focus === "comm" && comm
        ? [
            ["设备", device?.name || "—"],
            ["通讯", comm.name],
            ["角色", comm.role],
            ["协议", comm.protocol],
            ["地址", endpointLabel(comm)],
            ["采集表", bound?.name || "未绑定"],
          ]
        : [
            ["设备", device?.name || "—"],
            ["类型", device?.customType || device?.type || "—"],
            ["通讯", device ? device.comms.length : "—"],
          ];
  return (
    <div className="property-panel">
      {mode === "device" ? (
        <>
          <div className="property-heading">▾ 基本信息</div>
          <PropertyRows rows={rows} />
          <p className="note">
            {selection.focus === "template"
              ? "点的地址和类型在中间点表里修改。"
              : "在配置标签页编辑详细参数。"}
          </p>
        </>
      ) : (
        <>
          <div className="property-heading">▾ 策略参数</div>
          <div className="parameter-fields">
            {[
              ["初始 SOC (%)", "soc"],
              ["SOC 上限 (%)", "upper"],
              ["SOC 下限 (%)", "lower"],
              ["充放电功率 (kW)", "maxPower"],
              ["容量 (kWh)", "capacity"],
            ].map(([label, k]) => (
              <Field
                key={k}
                label={label}
                type="number"
                value={project.params[k]}
                change={(v) =>
                  setProject((p) => ({ ...p, params: { ...p.params, [k]: v } }))
                }
              />
            ))}
          </div>
          <div className="property-heading">▾ 数据绑定 · 教学设备</div>
          <PropertyRows
            rows={[
              ["读取", "BMS_01.SOC"],
              ["读取", "PCS_01.实际功率"],
              ["控制", "PCS_01.目标功率"],
            ]}
          />
          <div className="property-heading">▾ 仿真设置</div>
          <PropertyRows
            rows={[
              ["模式", "离线简化仿真"],
              ["时长", "24 h"],
              ["步长", "5 min"],
              ["功率约定", "正放电 / 负充电"],
            ]}
          />
          <p className="note">教学模型，不连接左侧设备的真实协议实例。</p>
        </>
      )}
    </div>
  );
}
function Model() {
  const { device, models, setModels, notify } = useLab();
  if (!device) return <div className="empty">请选择设备</div>;
  async function upload(file) {
    if (!file) return;
    if (
      !file.name.toLowerCase().endsWith(".glb") ||
      file.size > 20 * 1024 * 1024
    ) {
      notify("请选择不超过 20 MB 的内嵌资源 GLB", "错误");
      return;
    }
    try {
      const buffer = await file.arrayBuffer();
      if (
        buffer.byteLength < 12 ||
        new DataView(buffer).getUint32(0, true) !== 0x46546c67
      )
        throw Error();
      setModels((m) => ({ ...m, [device.id]: { buffer, name: file.name } }));
      notify("模型已加载，仅本次会话保留");
    } catch {
      notify("GLB 文件无效", "错误");
    }
  }
  return (
    <div className="model-panel">
      <ModelView type={device.type} model={models[device.id]} />
      <div className="model-footer">
        <label className="upload-button">
          <Upload size={14} />
          上传模型
          <input
            type="file"
            aria-label="上传 GLB 模型"
            accept=".glb"
            onChange={(e) => {
              upload(e.target.files[0]);
              e.target.value = "";
            }}
          />
        </label>
        <span>GLB · 示意视图</span>
      </div>
    </div>
  );
}
function ScriptEditor({ params }) {
  const {
    project,
    setProject,
    scriptId,
    setScriptId,
    run,
    stop,
    running,
    notify,
  } = useLab();
  const s =
    project.scripts.find((x) => x.id === (params.scriptId || scriptId)) ||
    project.scripts[0];
  useEffect(() => setScriptId(s.id), [s.id]);
  return (
    <div className="script-editor">
      <div className="panel-toolbar">
        <button disabled={running} onClick={run}>
          <Play className="green" />
          运行仿真
        </button>
        <button
          disabled={!running}
          onClick={() => {
            stop();
            notify("仿真已停止");
          }}
        >
          <Square className="red" />
          停止
        </button>
        <i />
        <span>离线仿真</span>
        <span className="muted">24 h / 5 min</span>
      </div>
      <CodeMirror
        value={s.source}
        height="100%"
        extensions={[javascript()]}
        onChange={(source) =>
          setProject((p) => ({
            ...p,
            scripts: p.scripts.map((x) =>
              x.id === s.id ? { ...x, source } : x,
            ),
          }))
        }
        className="code-editor"
        basicSetup={{
          lineNumbers: true,
          foldGutter: true,
          highlightActiveLine: true,
        }}
      />
      <div className="editor-foot">
        JavaScript · UTF-8 · Ctrl/Cmd + S 保存项目 · 原型脚本运行限制 5 秒
      </div>
    </div>
  );
}
function Results() {
  const { result, running } = useLab();
  if (!result)
    return (
      <div className="empty">
        <Code2 size={24} />
        <strong>{running ? "正在运行脚本…" : "运行脚本，查看仿真结果"}</strong>
        <span>修改 on_tick(ctx)，观察功率和 SOC 的变化。</span>
      </div>
    );
  const points = result.samples;
  const width = 900,
    height = 150;
  const path = (k) =>
    points
      .map(
        (s, i) =>
          `${i ? "L" : "M"}${45 + (s.hour / 24) * (width - 90)},${k === "soc" ? height - 20 - (s.soc / 100) * (height - 35) : height / 2 - (s.power / result.params.maxPower) * (height / 2 - 20)}`,
      )
      .join(" ");
  return (
    <div className="result-panel">
      <div className="chart-legend">
        <span className="blue">━ SOC (%)</span>
        <span className="green">━ 功率 (kW)</span>
        <span className="muted">简化仿真 · {points.length} 个采样点</span>
      </div>
      <svg
        viewBox={`0 0 ${width} ${height}`}
        role="img"
        aria-label="SOC与功率仿真结果曲线"
      >
        {[0, 25, 50, 75, 100].map((n) => (
          <g key={n}>
            <line
              x1="45"
              x2="855"
              y1={height - 20 - (n / 100) * (height - 35)}
              y2={height - 20 - (n / 100) * (height - 35)}
              stroke="#e7ecf1"
            />
            <text x="15" y={height - 17 - (n / 100) * (height - 35)}>
              {n}
            </text>
          </g>
        ))}
        <path d={path("soc")} stroke="#2284e5" fill="none" strokeWidth="2" />
        <path
          d={path("power")}
          stroke="#279255"
          fill="none"
          strokeWidth="1.5"
        />
        {[0, 3, 6, 9, 12, 15, 18, 21, 24].map((h) => (
          <text key={h} x={45 + (h / 24) * 810} y="147" textAnchor="middle">
            {String(h).padStart(2, "0")}:00
          </text>
        ))}
      </svg>
      <div className="metrics">
        <span>
          末端 SOC <strong>{points.at(-1).soc.toFixed(1)}%</strong>
        </span>
        <span>
          最大放电{" "}
          <strong>
            {Math.max(...points.map((s) => s.power)).toFixed(0)} kW
          </strong>
        </span>
        <span>
          最大充电{" "}
          <strong>
            {Math.abs(Math.min(...points.map((s) => s.power))).toFixed(0)} kW
          </strong>
        </span>
      </div>
    </div>
  );
}
function Logs({ params = {} }) {
  const { logs } = useLab();
  if (params.packets)
    return <div className="empty">真实协议引擎尚未接入，暂无通信报文。</div>;
  const list = params.errors ? logs.filter((l) => l.level === "错误") : logs;
  return (
    <div className="log-panel">
      <table className="data-table">
        <thead>
          <tr>
            <th>时间</th>
            <th>级别</th>
            <th>消息</th>
          </tr>
        </thead>
        <tbody>
          {list.map((l, i) => (
            <tr key={i} className={l.level === "错误" ? "error-row" : ""}>
              <td>{l.time}</td>
              <td>{l.level}</td>
              <td>{l.message}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {!list.length && <p className="note">暂无错误记录。</p>}
    </div>
  );
}
function DeviceDialog({ close, options }) {
  const { project, setProject, notify, openNode } = useLab();
  const [draft, setDraft] = useState({
      name: `Modbus ${options.role}模拟器`,
      folderId: options.folderId || "",
      host: options.role === "从站" ? "0.0.0.0" : "127.0.0.1",
      port: 502, unitId: 1, templateId: "",
      type: "PCS",
      customType: "",
      description: "",
    }),
    [error, setError] = useState("");
  return (
    <div className="dialog-backdrop">
      <form
        className="dialog"
        onSubmit={(e) => {
          e.preventDefault();
          const err = validateDevice(draft);
          setError(err);
          if (err) return;
          const id = crypto.randomUUID();
          const comm = blankComm(draft.type, {role:options.role,host:draft.host,port:Number(draft.port),unitId:Number(draft.unitId),templateId:draft.templateId});
          const communicationError = validateComm(comm);
          if (communicationError || !Number.isInteger(comm.unitId) || comm.unitId < 1 || comm.unitId > 247) { setError(communicationError || "Unit ID 应为 1–247 的整数"); return; }
          setProject((p) => ({
            ...p,
            devices: [...p.devices, { ...draft, id, comms: [comm] }],
          }));
          openNode(
            { focus: "comm", deviceId: id, commId: comm.id, initialTab: "通信配置" },
            commLabel(comm),
          );
          notify(
            draft.type === "EMS"
              ? "EMS 已创建，请填写目标地址并选择采集表"
              : "设备已创建，默认从站通讯，请选择采集表",
          );
          close();
        }}
      >
        <div className="dialog-title">
          新建 Modbus {options.role}模拟器
          <button type="button" aria-label="关闭" onClick={close}>
            <X size={16} />
          </button>
        </div>
        <div className="config-form">
          <Field
            label="模拟器名称"
            value={draft.name}
            change={(v) => setDraft((d) => ({ ...d, name: v }))}
          />
          <label>
            设备类型
            <select
              value={draft.type}
              onChange={(e) =>
                setDraft((d) => ({ ...d, type: e.target.value }))
              }
            >
              {types.map((t) => (
                <option key={t}>{t}</option>
              ))}
            </select>
          </label>
          {draft.type === "自定义" && (
            <Field
              label="自定义类型"
              value={draft.customType}
              change={(v) => setDraft((d) => ({ ...d, customType: v }))}
            />
          )}
          <Field
            label="说明"
            value={draft.description}
            change={(v) => setDraft((d) => ({ ...d, description: v }))}
          />
          <Field label={options.role === "从站" ? "绑定地址" : "目标地址"} value={draft.host} change={host => setDraft({...draft,host})}/>
          <Field label="TCP 端口" type="number" value={draft.port} change={port => setDraft({...draft,port})}/>
          <Field label="Unit ID（1–247）" type="number" value={draft.unitId} change={unitId => setDraft({...draft,unitId})}/>
          <label>Modbus 点表模型<select value={draft.templateId} onChange={e => setDraft({...draft,templateId:e.target.value})}><option value="">稍后配置</option>{project.templates.filter(t => t.protocol === 'Modbus TCP').map(t => <option key={t.id} value={t.id}>{t.name}</option>)}</select></label>
          <p className="tree-hint">原型当前提供 Modbus TCP 配置，创建后不会启动真实通信。</p>
          {error && (
            <p role="alert" className="error">
              {error}
            </p>
          )}
          <div className="dialog-actions">
            <button type="button" onClick={close}>
              取消
            </button>
            <button className="primary">创建模拟器</button>
          </div>
        </div>
      </form>
    </div>
  );
}
function formatTime(minutes) {
  return `${String(Math.floor(minutes / 60)).padStart(2, "0")}:${String(minutes % 60).padStart(2, "0")}`;
}
const components = {
  tree: Tree,
  editor: Editor,
  properties: Properties,
  model: Model,
  script: ScriptEditor,
  results: Results,
  logs: Logs,
};
createRoot(document.getElementById("root")).render(<App />);
