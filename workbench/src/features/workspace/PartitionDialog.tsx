import {useEffect,useRef,useState} from "react";
import {Button} from "@essim/ui/components/button";
import {EnumSelect} from "./EnumSelect";
import {WorkspaceStore,errorMessage} from "./model";
import type {PartitionPreviewResult,PartitionRequest} from "../../shared/api/contracts";
/** 已保存模型分组；预览不应用、不启动、不修改系统IP。 */
export function PartitionDialog({store,close}:{store:WorkspaceStore;close:()=>void}){
 const dialog=useRef<HTMLDialogElement>(null);
 const [source,setSource]=useState(store.view?.catalog.models[0]?.id??"");
 const [prefix,setPrefix]=useState("分组模拟器");const [size,setSize]=useState("1000");
 const [ip,setIp]=useState("127.0.0.1");const [port,setPort]=useState("1080");const [station,setStation]=useState("1");const [folder,setFolder]=useState("");
 const [preview,setPreview]=useState<PartitionPreviewResult|null>(null);const [busy,setBusy]=useState(false);const [error,setError]=useState("");
 const alive=useRef(true);const token=useRef<string|null>(null);
 useEffect(()=>{alive.current=true;dialog.current?.showModal();return()=>{alive.current=false;if(token.current)void store.client.discardPartition?.(token.current).catch(()=>{});};},[store]);
 const change=(setter:(v:string)=>void)=>(value:string)=>{setter(value);setPreview(null);setError("");};
 const prepare=async()=>{
  setBusy(true);setError("");setPreview(null);
  try{if(!store.client.previewPartition)throw Error("后端未提供分组创建");
   const input:PartitionRequest={source,batchId:crypto.randomUUID(),prefix,groupSize:Number(size),startIp:ip,port:Number(port),station:Number(station),folder:folder||null};
   if(!Number.isInteger(input.groupSize)||input.groupSize<1||!Number.isInteger(input.port)||input.port<1||input.port>65535||!Number.isInteger(input.station)||input.station<1||input.station>247)throw Error("组大小、端口或站号非法");
   const next=await store.client.previewPartition(input);if(alive.current){token.current=next.token;setPreview(next);}else{void store.client.discardPartition?.(next.token).catch(()=>{});}
  }catch(e){if(alive.current)setError(errorMessage(e));}finally{if(alive.current)setBusy(false);}
 };
 const create=async()=>{if(!preview?.canCreate)return;setBusy(true);setError("");try{if(!store.client.createPartition)throw Error("分组创建未连接");await store.client.createPartition(preview.token);await store.refresh();close();}catch(e){if(alive.current){setError(errorMessage(e));setPreview(null);}}finally{if(alive.current)setBusy(false);}};
 return <dialog ref={dialog} className="create-dialog partition-dialog" aria-label="按采集点分组创建" onCancel={e=>{if(busy)e.preventDefault();else close();}}>
  <h2>按采集点分组创建</h2><p className="muted">使用已保存模型，保留原数据区与地址。每组生成独立模型和单 Unit TCP 从站，创建后不自动应用或启动。</p>
  <fieldset disabled={busy} style={{border:0,padding:0,display:"grid",gap:10}}>
   <label>已保存源模型<EnumSelect label="分组源模型" value={source} onChange={change(setSource)} options={store.view?.catalog.models.map(m=>[m.id,m.name]as const)??[]}/></label>
   <label>名称前缀<input aria-label="分组名称前缀" value={prefix} onChange={e=>change(setPrefix)(e.target.value)}/></label>
   <label>每组采集点数<input aria-label="每组采集点数" type="number" min="1" value={size} onChange={e=>change(setSize)(e.target.value)}/></label>
   <label>起始本机 IPv4<input aria-label="分组起始IP" value={ip} onChange={e=>change(setIp)(e.target.value)}/></label>
   <label>统一端口<input aria-label="分组端口" type="number" min="1" max="65535" value={port} onChange={e=>change(setPort)(e.target.value)}/></label>
   <label>统一 Unit ID<input aria-label="分组站号" type="number" min="1" max="247" value={station} onChange={e=>change(setStation)(e.target.value)}/></label>
   <label>目标文件夹<EnumSelect label="分组目标文件夹" value={folder} onChange={change(setFolder)} options={[["","根目录"],...(store.view?.catalog.navigation?.folders.map(f=>[f.id,f.name]as const)??[])]}/></label>
  </fieldset>
  <p className="muted">IP逐组递增、端口相同。所有目标IP须已配置在本机；预检不占端口，启动时仍可能发生占用冲突。</p>
  {error&&<p role="alert" className="error">{error}</p>}
  {preview&&<section aria-label="分组结果预览" className="partition-preview"><strong>{preview.preview.totalPoints} 点 → {preview.preview.groups.length} 组</strong><p className="muted">预计暂存：{(preview.estimatedMemoryBytes/1024/1024).toFixed(1)} MiB（估计） · 本机IP：{preview.localIps.join("、")}</p>{!preview.canCreate&&<p role="alert" className="error">{preview.localIssues.length} 组IP未配置，无法创建；请修改分组参数或使用已配置的本机IP。</p>}<ol>{preview.preview.groups.slice(0,100).map(g=><li key={g.ip}>{g.name} · {g.ip}:{g.port} / Unit {g.station} · {g.points}点<br/><small>{g.ranges.slice(0,8).map(([a,s,e])=>`${a}[${s}～${e}]`).join("、")}{g.ranges.length>8?` 等${g.ranges.length}个范围`:""}</small>{preview.localIssues.filter(issue=>issue.ip===g.ip).map(issue=><p key={issue.ip} className="error">{issue.message}</p>)}</li>)}</ol>{preview.preview.groups.length>100&&<p>共{preview.preview.groups.length}组，此处展示前100组。</p>}</section>}
  <div className="dialog-actions"><Button variant="outline" disabled={busy} onClick={close}>取消</Button><Button variant="outline" disabled={busy||!source} onClick={()=>void prepare()}>预览分组</Button><Button disabled={busy||!preview?.canCreate} onClick={()=>void create()}>{busy?"处理中…":"确认创建"}</Button></div>
 </dialog>;
}
