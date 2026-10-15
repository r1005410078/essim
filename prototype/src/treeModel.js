import { preparePresortedFileTreeInput } from "@pierre/trees";
import { commLabel, endpointLabel } from "./domain.js";

// 标签必须是路径最后一段。类型和身份放在不可见字符里，避免把 ID 或扩展名画出来。
const MARK = {
  table: "\u2060",
  device: "\u2061",
  script: "\u2063",
  modbus: "\u200b",
  iec104: "\u200c",
  iec61850: "\u200d",
};
const HIDE_HEAD = "\u2064\u2062";
const MARKS = new Set(Object.values(MARK));

export function sanitizeTreeLabel(label) {
  const text = String(label ?? "")
    .replaceAll("/", "／")
    .replace(/[\u0000-\u001f\u2060-\u2064\u200b-\u200d]/g, "")
    .trim();
  return text || "未命名";
}

function hideKey(id) {
  let bits = HIDE_HEAD;
  const text = String(id);
  for (let i = 0; i < text.length; i += 1) {
    const code = text.charCodeAt(i);
    for (let bit = 15; bit >= 0; bit -= 1)
      bits += (code >> bit) & 1 ? "\u2064" : "\u2062";
  }
  return bits;
}

function segment(mark, id, label) {
  return mark + hideKey(id) + sanitizeTreeLabel(label);
}

// 视图里的目录路径以 / 结尾，动作表使用去掉结尾斜杠的路径。
export function canonicalTreePath(path) {
  const text = String(path ?? "");
  return text.endsWith("/") ? text.slice(0, -1) : text;
}

export function visibleTreeLabel(path) {
  const base = canonicalTreePath(path).split("/").pop() || "";
  let index = MARKS.has(base[0]) ? 1 : 0;
  if (base.startsWith(HIDE_HEAD, index)) {
    index += HIDE_HEAD.length;
    while (base[index] === "\u2062" || base[index] === "\u2064") index += 1;
  }
  return base.slice(index);
}

function protocolMark(protocol) {
  if (protocol === "IEC 60870-5-104") return MARK.iec104;
  if (protocol === "IEC 61850 MMS") return MARK.iec61850;
  return MARK.modbus;
}

function treeSignature(project, mode) {
  if (mode === "strategy")
    return (
      "strategy\n" +
      (project.scripts || []).map((s) => `${s.id}\0${s.name}`).join("\n")
    );
  const templates = (project.templates || [])
    .map((t) => [t.id, t.name, t.protocol].join("\0"))
    .join("\n");
  const devices = (project.devices || [])
    .map((d) => {
      const comms = (d.comms || [])
        .map((c) =>
          [c.id, c.name, c.protocol, c.role, c.host, c.port, c.templateId].join(
            "\0",
          ),
        )
        .join("\n");
      return [d.id, d.name, d.type, comms].join("\n");
    })
    .join("\n---\n");
  return `device\n${templates}\n===\n${devices}`;
}

export function buildResourceTree(project, mode) {
  const paths = [];
  const directories = [];
  const actions = new Map();
  const library = new Map();
  const device = new Map();
  const comm = new Map();
  const bound = new Map();
  const script = new Map();
  const addDir = (path, action) => {
    paths.push(`${path}/`);
    directories.push(path);
    actions.set(path, action);
  };
  const addFile = (path, action) => {
    paths.push(path);
    actions.set(path, action);
  };

  if (mode === "strategy") {
    for (const item of project.scripts || []) {
      const path = segment(MARK.script, item.id, item.name);
      script.set(item.id, path);
      addFile(path, {
        kind: "script",
        open: { scriptId: item.id },
      });
    }
    addDir("场景数据 24 h", { kind: "folder" });
    addDir("运行记录", { kind: "folder" });
  } else {
    const templates = project.templates || [];
    if (templates.length === 0) addDir("点表库", { kind: "library" });
    else {
      directories.push("点表库");
      actions.set("点表库", { kind: "library" });
      for (const template of templates) {
        const path = `点表库/${segment(MARK.table, template.id, template.name)}`;
        library.set(template.id, path);
        addFile(path, {
          kind: "template",
          title: template.protocol,
          open: {
            sel: {
              focus: "template",
              templateId: template.id,
              initialTab: "点表",
            },
            title: template.name,
          },
        });
      }
    }
    const devices = project.devices || [];
    if (devices.length === 0) addDir("模拟设备", { kind: "devices" });
    else {
      directories.push("模拟设备");
      actions.set("模拟设备", { kind: "devices" });
    }
    for (const item of devices) {
      const devicePath = `模拟设备/${segment(MARK.device, item.id, item.name)}`;
      directories.push(devicePath);
      device.set(item.id, devicePath);
      actions.set(devicePath, {
        kind: "device",
        deviceId: item.id,
        open: {
          sel: { focus: "device", deviceId: item.id },
          title: item.name,
        },
      });
      for (const itemComm of item.comms || []) {
        const endpoint = `${itemComm.host}:${itemComm.port}`;
        const commPath = `${devicePath}/${segment(protocolMark(itemComm.protocol), itemComm.id, endpoint)}`;
        directories.push(commPath);
        comm.set(itemComm.id, commPath);
        actions.set(commPath, {
          kind: "comm",
          title: `${itemComm.protocol} · ${endpointLabel(itemComm)}`,
          open: {
            sel: {
              focus: "comm",
              deviceId: item.id,
              commId: itemComm.id,
            },
            title: commLabel(itemComm),
          },
        });
        const linked = templates.find((t) => t.id === itemComm.templateId);
        const tablePath = `${commPath}/${segment(MARK.table, `${itemComm.id}:table`, linked ? linked.name : "未绑定")}`;
        bound.set(itemComm.id, tablePath);
        addFile(tablePath, {
          kind: linked ? "bound" : "unbound",
          open: linked
            ? {
                sel: {
                  focus: "template",
                  templateId: linked.id,
                  deviceId: item.id,
                  commId: itemComm.id,
                  initialTab: "点表",
                },
                title: linked.name,
              }
            : {
                sel: {
                  focus: "comm",
                  deviceId: item.id,
                  commId: itemComm.id,
                  initialTab: "通信配置",
                },
                title: commLabel(itemComm),
              },
        });
      }
    }
  }

  return {
    paths,
    prepared: preparePresortedFileTreeInput(paths),
    directories,
    actions,
    library,
    device,
    comm,
    bound,
    script,
    signature: treeSignature(project, mode),
  };
}

export function selectionPath(tree, selection) {
  if (!selection) return null;
  if (selection.focus === "template" && selection.commId)
    return tree.bound.get(selection.commId) || null;
  if (selection.focus === "template")
    return tree.library.get(selection.templateId) || null;
  if (selection.focus === "comm") return tree.comm.get(selection.commId) || null;
  if (selection.focus === "device")
    return tree.device.get(selection.deviceId) || null;
  return null;
}

function svgUrl(svg) {
  return `url("data:image/svg+xml,${encodeURIComponent(svg)}")`;
}

const modbusSvg = `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 14 14"><rect width="14" height="14" rx="2" fill="#0e6b4a"/><path d="M3.1 11V3.2h1.7L7 7.6 9.2 3.2H10.9V11H9.4V6.3L7.2 9.6 5 6.3V11Z" fill="#fff"/></svg>`;
export { modbusSvg as modbusBadgeSvg };
const iec104Svg = `<svg xmlns="http://www.w3.org/2000/svg" width="22" height="14" viewBox="0 0 22 14"><rect width="22" height="14" rx="2" fill="#1d5fbf"/><text x="11" y="10.4" text-anchor="middle" fill="#fff" font-size="8" font-weight="700" font-family="ui-sans-serif,sans-serif">104</text></svg>`;
const iec61850Svg = `<svg xmlns="http://www.w3.org/2000/svg" width="34" height="14" viewBox="0 0 34 14"><rect width="34" height="14" rx="2" fill="#8a4b08"/><text x="17" y="10.2" text-anchor="middle" fill="#fff" font-size="7" font-weight="700" font-family="ui-sans-serif,sans-serif">61850</text></svg>`;
const deviceSvg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 14 14"><rect x="1.2" y="1.2" width="11.6" height="11.6" rx="1.4" fill="none" stroke="black" stroke-width="1.4"/><path d="M1.2 5h11.6" stroke="black" stroke-width="1.2"/></svg>`;
const folderClosedSvg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path fill="black" d="M1.6 3.2A1.2 1.2 0 0 1 2.8 2h3.05l1.2 1.35H13.2A1.2 1.2 0 0 1 14.4 4.55v8.05a1.2 1.2 0 0 1-1.2 1.2H2.8a1.2 1.2 0 0 1-1.2-1.2V3.2Z"/></svg>`;
const folderOpenSvg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path fill="black" d="M1.5 2.2A1 1 0 0 1 2.5 1.2h3l1.05 1.15H13.4a1 1 0 0 1 1 1V5.3H1.5V2.2Z"/><path fill="black" d="M.7 7.5h14.6L13.7 13.6a1 1 0 0 1-.96.75H2.4a1 1 0 0 1-.96-.75L.7 7.5Z"/></svg>`;

function folderIcon(mark, svg, width) {
  return `[data-item-type="folder"][data-item-path*="${mark}"] > [data-item-section="content"] {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}
[data-item-type="folder"][data-item-path*="${mark}"] > [data-item-section="content"]::before {
  content: "";
  width: ${width}px;
  height: 14px;
  flex: none;
  background: ${svgUrl(svg)} center / contain no-repeat;
}
[data-item-type="folder"][data-item-path*="${mark}"] > [data-item-section="content"] > * {
  min-width: 0;
}`;
}

export const resourceTreeCss = `
:host { color-scheme: light; display: block; height: 100%; }
${folderIcon(MARK.modbus, modbusSvg, 14)}
${folderIcon(MARK.iec104, iec104Svg, 22)}
${folderIcon(MARK.iec61850, iec61850Svg, 34)}
[data-item-type="folder"][data-item-path*="${MARK.device}"]:not([data-item-path*="${MARK.modbus}"]):not([data-item-path*="${MARK.iec104}"]):not([data-item-path*="${MARK.iec61850}"]) > [data-item-section="content"] {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}
[data-item-type="folder"][data-item-path*="${MARK.device}"]:not([data-item-path*="${MARK.modbus}"]):not([data-item-path*="${MARK.iec104}"]):not([data-item-path*="${MARK.iec61850}"]) > [data-item-section="content"]::before {
  content: "";
  width: 14px;
  height: 14px;
  flex: none;
  background: currentColor;
  -webkit-mask: ${svgUrl(deviceSvg)} center / contain no-repeat;
  mask: ${svgUrl(deviceSvg)} center / contain no-repeat;
}
[data-item-type="folder"][data-item-path="点表库"],
[data-item-type="folder"][data-item-path="点表库/"],
[data-item-type="folder"][data-item-path="模拟设备"],
[data-item-type="folder"][data-item-path="模拟设备/"] {
  --essim-folder-closed: ${svgUrl(folderClosedSvg)};
  --essim-folder-open: ${svgUrl(folderOpenSvg)};
}
[data-item-type="folder"][data-item-path="点表库"] > [data-item-section="content"],
[data-item-type="folder"][data-item-path="点表库/"] > [data-item-section="content"],
[data-item-type="folder"][data-item-path="模拟设备"] > [data-item-section="content"],
[data-item-type="folder"][data-item-path="模拟设备/"] > [data-item-section="content"] {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}
[data-item-type="folder"][data-item-path="点表库"] > [data-item-section="content"]::before,
[data-item-type="folder"][data-item-path="点表库/"] > [data-item-section="content"]::before,
[data-item-type="folder"][data-item-path="模拟设备"] > [data-item-section="content"]::before,
[data-item-type="folder"][data-item-path="模拟设备/"] > [data-item-section="content"]::before {
  content: "";
  width: 14px;
  height: 14px;
  flex: none;
  -webkit-mask: var(--essim-folder-closed) center / contain no-repeat;
  mask: var(--essim-folder-closed) center / contain no-repeat;
}
[aria-expanded="true"][data-item-path="点表库"] > [data-item-section="content"]::before,
[aria-expanded="true"][data-item-path="点表库/"] > [data-item-section="content"]::before,
[aria-expanded="true"][data-item-path="模拟设备"] > [data-item-section="content"]::before,
[aria-expanded="true"][data-item-path="模拟设备/"] > [data-item-section="content"]::before {
  -webkit-mask: var(--essim-folder-open) center / contain no-repeat;
  mask: var(--essim-folder-open) center / contain no-repeat;
}
[data-item-type="folder"][data-item-path="点表库"] > [data-item-section="content"]::before,
[data-item-type="folder"][data-item-path="点表库/"] > [data-item-section="content"]::before {
  background: #c98612;
}
[data-item-type="folder"][data-item-path="模拟设备"] > [data-item-section="content"]::before,
[data-item-type="folder"][data-item-path="模拟设备/"] > [data-item-section="content"]::before {
  background: #2a7fbf;
}
[data-icon-name="file-tree-icon-file"] { color: #5c6e82; }
[data-item-selected="true"] [data-icon-name="file-tree-icon-file"] { color: #146abf; }
[data-item-section="decoration"]:has(span[title]) {
  position: absolute;
  inset: 0;
  opacity: 0;
}
[data-item-section="decoration"]:has(span[title]) > span {
  position: absolute;
  inset: 0;
  max-width: none;
}
`;

export const resourceTreeIcons = {
  set: "minimal",
  colored: false,
  spriteSheet: `<svg aria-hidden="true" width="0" height="0">
    <symbol id="essim-simulator" viewBox="0 0 16 16"><rect x="1" y="1" width="12" height="12" rx="3" fill="#e0f1ea" stroke="#278065"/><path d="M3.5 10V4.2H5L7 7.5l2-3.3h1.5V10H9V6.6L7 9.5 5 6.6V10Z" fill="#176a50"/><circle cx="12.5" cy="12.5" r="3.5" fill="#278065" stroke="#f8fafc"/><path d="M12.5 10.5v4m-1.5-1.5 1.5 1.5 1.5-1.5" stroke="white" fill="none" stroke-width="1"/></symbol>
    <symbol id="essim-master" viewBox="0 0 16 16"><rect x="1" y="1" width="12" height="12" rx="3" fill="#e5edff" stroke="#3974c7"/><path d="M3.5 10V4.2H5L7 7.5l2-3.3h1.5V10H9V6.6L7 9.5 5 6.6V10Z" fill="#245daa"/><circle cx="12.5" cy="12.5" r="3.5" fill="#3974c7" stroke="#f8fafc"/><path d="M12.5 14.5v-4m-1.5 1.5 1.5-1.5 1.5 1.5" stroke="white" fill="none" stroke-width="1"/></symbol>
    <symbol id="essim-wifi-offline" viewBox="0 0 16 16"><g fill="none" stroke="#94a3b8" stroke-width="1.4" stroke-linecap="round"><path d="M2 5.5a9 9 0 0 1 12 0M4.3 8a5.6 5.6 0 0 1 7.4 0M6.5 10.5a2.3 2.3 0 0 1 3 0M2 2l12 12"/></g><circle cx="8" cy="13" r=".9" fill="#94a3b8"/></symbol>
    <symbol id="essim-table" viewBox="0 0 16 16">
      <path fill="currentColor" d="M2 3.2A1.2 1.2 0 0 1 3.2 2h9.6A1.2 1.2 0 0 1 14 3.2v9.6a1.2 1.2 0 0 1-1.2 1.2H3.2A1.2 1.2 0 0 1 2 12.8V3.2Zm1.4.2v2.2h9.2V3.4H3.4Zm0 3.6v2.2h3.6V7H3.4Zm5 0v2.2h4.2V7H8.4ZM3.4 10.6v2h3.6v-2H3.4Zm5 0v2h4.2v-2H8.4Z"/>
    </symbol>
    <symbol id="essim-script" viewBox="0 0 16 16">
      <path fill="currentColor" d="M5.1 4.2 2.3 8l2.8 3.8-.9.7L1 8l3.2-4.5 1 .7Zm5.8 0 .9-.7L15 8l-3.2 4.5-.9-.7L13.7 8 10.9 4.2ZM7.2 13l2.4-10 .9.2-2.4 10-.9-.2Z"/>
    </symbol>
  </svg>`,
  byFileNameContains: {
    [MARK.modbus]: { name: "essim-simulator", width: 16, height: 16, viewBox: "0 0 16 16" },
    [MARK.device]: { name: "essim-master", width: 16, height: 16, viewBox: "0 0 16 16" },
    [MARK.table]: { name: "essim-table", width: 16, height: 16, viewBox: "0 0 16 16" },
    [MARK.script]: { name: "essim-script", width: 16, height: 16, viewBox: "0 0 16 16" },
  },
};

// 用户目录仅负责组织，路径中的隐藏 ID 允许同名文件并保持引用身份。
export function buildProjectFileTree(project) {
  const paths = [], actions = new Map(), directories = [];
  const folders = project.folders || [];
  function visit(parentId, parentPath, ancestors = new Set()) {
    for (const folder of folders.filter(f => (f.parentId || '') === parentId)) {
      if (ancestors.has(folder.id)) continue;
      const path = `${parentPath}${segment('', folder.id, folder.name)}`;
      paths.push(`${path}/`); directories.push(path);
      actions.set(path, {kind:'folder', item:folder});
      visit(folder.id, `${path}/`, new Set([...ancestors, folder.id]));
    }
    for (const [kind, items, mark] of [['template',project.templates || [],MARK.table],['device',project.devices || [],MARK.modbus]]) {
      for (const item of items.filter(f => (f.folderId || '') === parentId)) {
        const fileMark = kind === 'device' && item.comms?.[0]?.role === '主站' ? MARK.device : mark;
        const path = `${parentPath}${segment(fileMark,item.id,item.name)}`;
        paths.push(path); actions.set(path,{kind,item});
      }
    }
  }
  visit('', '');
  return {paths, actions, directories, prepared:preparePresortedFileTreeInput(paths), signature:JSON.stringify(paths)};
}

// 状态位固定在节点最右侧，文件夹图标独立于展开箭头。
export const projectTreeCss = resourceTreeCss.split('[data-item-section="decoration"]:has(span[title])')[0] + `
[data-item-section="decoration"] { justify-content: flex-end; padding-right: 4px; }
[data-item-section="action"] { display: none; }
[data-item-type="folder"] > [data-item-section="content"] { display: flex; align-items: center; gap: 6px; }
[data-item-type="folder"] > [data-item-section="content"]::before {
  content: ""; width: 14px; height: 14px; flex: none; background: #718399;
  -webkit-mask: ${svgUrl(folderClosedSvg)} center / contain no-repeat;
  mask: ${svgUrl(folderClosedSvg)} center / contain no-repeat;
}
[data-item-type="folder"][aria-expanded="true"] > [data-item-section="content"]::before {
  -webkit-mask-image: ${svgUrl(folderOpenSvg)}; mask-image: ${svgUrl(folderOpenSvg)};
}
`;
