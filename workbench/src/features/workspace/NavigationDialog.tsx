import {FolderGlyph} from "./folderIcons";
import {FolderAppearance} from "./FolderAppearance";
import type {FolderScope} from "../../shared/api/contracts";
import { WorkbenchDialog, DialogBody, DialogFooter } from "@essim/ui/components/workbench-dialog";
import { useState } from "react";
import { EnumSelect } from "./EnumSelect";
import { WorkspaceStore, errorMessage, nextStation, type Target } from "./model";
import { editNavigationUnit, type UnitNavigationAction } from "./navigationActions";

export type NavigationRequest = { kind: "copy" | "delete" | "rename" | "move" | "newFolder" | UnitNavigationAction; target?: Target; scope?:FolderScope; restoreFocus?:()=>void };
/** 导航操作只提交稳定身份；单元编辑与整机生命周期操作分开。 */
export function NavigationDialog({ store, request, close, open, removed }: {
  store: WorkspaceStore; request: NavigationRequest; close: () => void; open: (t: Target) => void; removed: (id: string) => void;
}) {
  const catalog = store.view!.catalog;
  const endpoint = request.target?.kind === "endpoint" ? store.endpoint(request.target.id) : undefined;
  const unit = endpoint?.units.find(u => u.id === request.target?.unit);
  const unitAction = request.kind === "newUnit" || request.kind === "renameUnit" || request.kind === "deleteUnit";
  const folder = request.target?.kind === "folder" ? catalog.navigation?.folders.find(f => f.id === request.target!.id) : undefined;
  const scope=folder?.scope??request.scope??(request.target?.kind==="model"?"models":"simulators");
  const [icon,setIcon]=useState(folder?.icon??"folder");const [color,setColor]=useState(folder?.color??"#477fbc");
  const folderEditing=request.kind==="newFolder"||request.kind==="rename"&&!!folder;
  const suggestedStation = endpoint ? nextStation(endpoint) : undefined;
  const [name, setName] = useState(unitAction ? (request.kind === "newUnit" ? `Unit ${suggestedStation ?? ""}` : unit?.name ?? "") : request.kind==="newFolder"?"新建文件夹":endpoint?.name ?? folder?.name ?? "新建文件夹");
  const [station, setStation] = useState(String(suggestedStation ?? ""));
  const [model, setModel] = useState(unit?.model.id ?? catalog.models[0]?.id ?? "");
  const [parent, setParent] = useState(request.kind==="newFolder"?folder?.id??"":folder?folder.parent??"":endpoint?catalog.navigation?.members[endpoint.id]??"":request.target?.kind==="model"?catalog.navigation?.modelMembers?.[request.target.id]??"":"");
  const parentFolders=(catalog.navigation?.folders??[]).filter(candidate=>{
    if((candidate.scope??"simulators")!==scope)return false;
    if(!folder||request.kind==="newFolder")return true;
    const seen=new Set<string>();let ancestor:typeof candidate|undefined=candidate;
    while(ancestor){if(ancestor.id===folder.id||seen.has(ancestor.id))return false;seen.add(ancestor.id);ancestor=catalog.navigation?.folders.find(item=>item.id===ancestor!.parent);}
    return true;
  });
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const title = { copy: "复制模拟器", delete: folder ? "删除空文件夹" : "删除模拟器", rename: folder?"编辑文件夹":"重命名", move: "移动到", newFolder: "新建文件夹", newUnit: "新增 Unit", renameUnit: "重命名单元", deleteUnit: "删除单元" }[request.kind];
  const naming = ["rename", "newFolder", "newUnit", "renameUnit"].includes(request.kind);

  async function submit() {
    setBusy(true); setError("");
    try {
      if (unitAction) {
        if (!request.target) throw Error("请选择所属模拟器");
        const id = editNavigationUnit(store, request.kind as UnitNavigationAction, request.target, { name, station: Number(station), model });
        open({ kind: "endpoint", id: request.target.id, ...(id ? { unit: id } : {}) });
      } else {
        // 即使未来菜单接入错误，也不能把带Unit身份的请求降级成整机操作。
        if (request.target?.unit) throw Error("单元操作不能作用于整个模拟器");
        if (request.kind === "copy" && endpoint) { const id = crypto.randomUUID(); await store.execute({ kind: "copyEndpoint", source: endpoint.id, id }); open({ kind: "endpoint", id }); }
        else if (request.kind === "delete") {
          if (endpoint) { await store.execute({ kind: "deleteEndpoint", id: endpoint.id }); removed(endpoint.id); }
          else if (folder) await store.execute({ kind: "deleteFolder", id: folder.id });
          else throw Error("对象已不存在");
        } else if (request.kind === "newFolder") await store.execute({ kind: "saveFolder", folder: { id: crypto.randomUUID(), name: name.trim(), parent: parent || null,scope,icon,color } });
        else if (folder) await store.execute({ kind: "saveFolder", folder: { ...folder, icon,color,name: request.kind === "rename" ? name.trim() : folder.name, parent: request.kind === "move" || folderEditing ? parent || null : folder.parent } });
        else if(request.target?.kind==="model"&&request.kind==="move") await store.execute({kind:"moveNavigation",nodes:[{kind:"model",id:request.target.id}],scope:"models",parent:parent||null});
        else if (endpoint && request.kind === "move") await store.execute({ kind: "moveEndpoint", id: endpoint.id, parent: parent || null });
        else if (endpoint) { store.edit({ ...store.endpoint(endpoint.id)!, name: name.trim() }); open({ kind: "endpoint", id: endpoint.id }); }
        else throw Error("对象已不存在");
      }
      close();
    } catch (e) { setError(errorMessage(e)); } finally { setBusy(false); }
  }

  return <WorkbenchDialog onCancel={close} title={title} busy={busy || store.busy} initialWidth={folderEditing?440:undefined} restoreFocus={request.restoreFocus}>
    <form onSubmit={event => { event.preventDefault(); void submit(); }}>
      <DialogBody>
        {request.kind === "copy" && <p>复制当前保存配置，生成新模拟器及Unit身份，不带应用和值。{endpoint && store.drafts.has(endpoint.id) && "源有未保存修改，本次复制不包含这些修改。"}</p>}
        {request.kind === "delete" && <p>{endpoint ? `确认释放资源后删除 ${endpoint.name} 及其Unit会话值，保留模型和设备。` : "只允许删除空文件夹，请先移出所有内容。"}</p>}
        {request.kind === "deleteUnit" && <p>从配置草稿中删除 {unit?.name ?? "该单元"}，同时丢弃该单元的未保存点表修改。保存并应用后生效。</p>}
        {folderEditing?<div className="folder-dialog-fields">
          <label className="folder-name-row"><span>名称</span><span className="folder-name-input"><FolderGlyph icon={icon} color={color} size={18}/><input aria-label="名称" autoFocus required value={name} disabled={busy||store.busy} onChange={e=>setName(e.target.value)}/></span></label>
          <label><span>位置</span><EnumSelect label="目标文件夹" disabled={busy||store.busy} value={parent} onChange={setParent} options={[["",`${scope==="models"?"采集模型":"模拟器"} / 根目录`],...parentFolders.map(f=>[f.id,f.name] as const)]}/></label>
          <FolderAppearance icon={icon} color={color} onIcon={setIcon} onColor={setColor} disabled={busy||store.busy}/>
        </div>:naming&&<label>名称<input autoFocus required value={name} disabled={busy} onChange={e=>setName(e.target.value)}/></label>}
        {request.kind === "newUnit" && <>
          <label>站号<input required type="number" min="1" max="247" value={station} disabled={busy} onChange={e => setStation(e.target.value)} /></label>
          <label>采集模型<EnumSelect label="采集模型" disabled={busy} value={model} onChange={setModel} options={catalog.models.map(m => [m.id, m.name] as const)} /></label>
        </>}
        {(request.kind === "move") && <label>目标文件夹<EnumSelect label="目标文件夹" disabled={busy} value={parent} onChange={setParent} options={[["", "根目录"], ...parentFolders.map(f => [f.id, `${f.name} · ${f.id.slice(0, 8)}`] as const)]} /></label>}
        {(request.kind === "rename" && endpoint || request.kind === "renameUnit" || request.kind === "newUnit") && <p>修改保留在模拟器配置草稿中，保存并应用后生效。</p>}
        {error && <p role="alert" className="error">{error}</p>}
      </DialogBody>
      <DialogFooter>
        <button type="button" disabled={busy || store.busy} onClick={close}>取消</button>
        <button type="submit" disabled={busy || store.busy || (naming && !name.trim()) || (request.kind === "newUnit" && !model)}>确认</button>
      </DialogFooter>
    </form>
  </WorkbenchDialog>;
}
