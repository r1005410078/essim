import { WorkbenchDialog, DialogBody, DialogFooter } from "@essim/ui/components/workbench-dialog";
import { useEffect, useRef, useState } from "react";
import { EnumSelect } from "./EnumSelect";
import { WorkspaceStore, errorMessage, type Target } from "./model";
export type NavigationRequest={kind:"copy"|"delete"|"rename"|"move"|"newFolder";target?:Target};
/** 导航操作只提交稳定身份；路径改变不关闭文档。 */
export function NavigationDialog({store,request,close,open,removed}:{store:WorkspaceStore;request:NavigationRequest;close:()=>void;open:(t:Target)=>void;removed:(id:string)=>void}) {
 const ref=useRef<HTMLDialogElement>(null);const catalog=store.view!.catalog;
 const endpoint=request.target?.kind==="endpoint" ? catalog.endpoints.find(e=>e.id===request.target!.id):undefined;
 const folder=request.target?.kind==="folder" ? catalog.navigation?.folders.find(f=>f.id===request.target!.id):undefined;
 const [name,setName]=useState(endpoint?.name ?? folder?.name ?? "新建文件夹");
 const [parent,setParent]=useState((request.kind==="newFolder" ? folder?.id : folder?.parent) ?? (endpoint ? catalog.navigation?.members[endpoint.id] : request.target?.kind==="folder" ? request.target.id : "") ?? "");
 const [error,setError]=useState("");const [busy,setBusy]=useState(false);
 const title={copy:"复制模拟器",delete:folder?"删除空文件夹":"删除模拟器",rename:"重命名",move:"移动到",newFolder:"新建文件夹"}[request.kind];
 async function submit(){setBusy(true);setError("");try{
  if(request.kind==="copy" && endpoint){const id=crypto.randomUUID();await store.execute({kind:"copyEndpoint",source:endpoint.id,id});open({kind:"endpoint",id});}
  else if(request.kind==="delete"){
   if(endpoint){await store.execute({kind:"deleteEndpoint",id:endpoint.id});removed(endpoint.id);}
   else if(folder) await store.execute({kind:"deleteFolder",id:folder.id});
  } else if(request.kind==="newFolder") await store.execute({kind:"saveFolder",folder:{id:crypto.randomUUID(),name,parent:parent || null}});
  else if(folder) await store.execute({kind:"saveFolder",folder:{...folder,name:request.kind==="rename"?name:folder.name,parent:request.kind==="move"?parent || null:folder.parent}});
  else if(endpoint && request.kind==="move") await store.execute({kind:"moveEndpoint",id:endpoint.id,parent:parent || null});
  else if(endpoint){store.edit({...store.endpoint(endpoint.id)!,name});open({kind:"endpoint",id:endpoint.id});}
  close();
 }catch(e){setError(errorMessage(e));}finally{setBusy(false);}}
 return <WorkbenchDialog ref={ref} onCancel={e=>{e.preventDefault();e.stopPropagation();if(!busy)close();}} title={<>{title}</>} busy={busy}><DialogBody>
 {request.kind==="copy" && <p>复制当前保存配置，生成新模拟器及Unit身份，不带应用和值。{endpoint && store.drafts.has(endpoint.id) && "源有未保存修改，本次复制不包含这些修改。"}</p>}
 {request.kind==="delete" && <p>{endpoint ? `确认释放资源后删除 ${endpoint.name} 及其Unit会话值，保留模型和设备。` : "只允许删除空文件夹，请先移出所有内容。"}</p>}
 {(request.kind==="rename" || request.kind==="newFolder") && <label>名称<input autoFocus value={name} onChange={e=>setName(e.target.value)} /></label>}
 {(request.kind==="move" || request.kind==="newFolder") && <label>目标文件夹<EnumSelect label="目标文件夹" value={parent} onChange={setParent} options={[["","根目录"],...(catalog.navigation?.folders ?? []).map(f=>[f.id,`${f.name} · ${f.id.slice(0,8)}`] as const)]} /></label>}
 {request.kind==="rename" && endpoint && <p>重命名保留在完整模拟器草稿中，使用保存提交。</p>}
 {error && <p role="alert" className="error">{error}</p>}</DialogBody><DialogFooter><button disabled={busy} onClick={close}>取消</button><button disabled={busy || ((request.kind==="rename" || request.kind==="newFolder") && !name.trim())} onClick={()=>void submit()}>确认</button></DialogFooter>
 </WorkbenchDialog>;
}
