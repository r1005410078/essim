import {useEffect,useRef,useState,useSyncExternalStore} from "react";
import {Button} from "@essim/ui/components/button";
import {EnumSelect} from "./EnumSelect";
import {errorMessage,type WorkspaceStore} from "./model";
import type {SheetViews} from "./sheets";
import type {BatchOperation,BatchScope,BatchView,DraftDisposition,LifecyclePreview} from "../../shared/api/contracts";
const labels:Record<string,string>={pending:"待调度",running:"执行中",uncertain:"结果未确认",succeeded:"成功",failed:"失败",skipped:"跳过",cancelled:"取消未执行"};
/** 任务归属后端会话；页面只提交、查询与取消后续项。 */
export function BatchPanel({store,sheets}:{store:WorkspaceStore;sheets:SheetViews}){
 useSyncExternalStore(store.subscribe,store.snapshot);useSyncExternalStore(sheets.subscribe,sheets.snapshot);
 const [operation,setOperation]=useState<BatchOperation>("apply");const [scope,setScope]=useState("selected");const [selected,setSelected]=useState<string[]>([]);const [concurrency,setConcurrency]=useState(4);const [discard,setDiscard]=useState<string[]>([]);
 const [preview,setPreview]=useState<LifecyclePreview>();const [view,setView]=useState<BatchView|null>(null);const [busy,setBusy]=useState(false);const [error,setError]=useState("");const [fingerprint,setFingerprint]=useState("");
 const previewRef=useRef(preview);previewRef.current=preview;
 useEffect(()=>()=>{const id=previewRef.current?.plan.id;if(id)void store.client.discardBatch?.(id).catch(()=>{});},[store]);
 const catalog=store.view?.catalog;const endpoints=catalog?.endpoints??[];
 const pending=(id:string)=>{const endpoint=endpoints.find(e=>e.id===id);return endpoint?.role==="master"?endpoint.units.reduce((n,u)=>{const model=catalog?.values[u.id]?.model;return n+(model?sheets.pendingMaster(u.id,model).length:0);},0):0;};
 const drafts=():DraftDisposition=>({unsaved:endpoints.filter(e=>store.drafts.has(e.id)||e.units.some(u=>store.modelDrafts.get(u.model.id)?.dirty)).map(e=>e.id),retainWrites:endpoints.filter(e=>pending(e.id)>0&&!discard.includes(e.id)).map(e=>e.id)});
 const currentFingerprint=()=>JSON.stringify([drafts(),endpoints.map(e=>[e.id,pending(e.id)]),discard]);
 const active=!!view&&!view.finished;
 useEffect(()=>{let disposed=false,pending=false;const query=async()=>{if(pending)return;pending=true;try{const v=await store.client.queryBatch?.();if(!disposed)setView(v??null);}catch(e){if(!disposed)setError(errorMessage(e));}finally{pending=false;}};void query();const timer=setInterval(()=>void query(),500);return()=>{disposed=true;clearInterval(timer);};},[store]);
 const run=async(action:()=>Promise<void>)=>{setBusy(true);setError("");try{await action();}catch(e){setError(errorMessage(e));}finally{setBusy(false);}};
 const plan=()=>run(async()=>{const target:BatchScope=scope==="all"?{kind:"all"}:scope==="selected"?{kind:"selected",ids:selected}:{kind:"folder",id:scope.slice(7)};const p=await store.client.previewBatch?.({id:crypto.randomUUID(),operation,scope:target,concurrency},drafts());if(!p)throw Error("批量服务不可用");setPreview(p);setFingerprint(currentFingerprint());});
 const start=()=>run(async()=>{if(!preview)return;if(currentFingerprint()!==fingerprint){setPreview(undefined);throw Error("草稿已变化，请重新预览");}await store.client.startBatch?.(preview.plan.id);if(preview.plan.operation==="apply")for(const t of preview.plan.targets){if(discard.includes(t.endpoint)&&!preview.items.find(i=>i.endpoint===t.endpoint)?.reason){const e=endpoints.find(e=>e.id===t.endpoint);for(const u of e?.units??[])sheets.discardMaster(u.id);}}setPreview(undefined);setView(await store.client.queryBatch?.()??null);});
 const invalidate=()=>setPreview(undefined);
 return <section className="batch-panel">
  <div className="document-bar"><strong>批量任务</strong><span className="spacer"/><span>{active?`后台执行 · 占用 ${view.occupied} 槽`:view?"任务已结束":"无活动任务"}</span></div>
  <div className="batch-toolbar">
   <label>操作 <EnumSelect value={operation} label="批量操作" options={[["apply","应用保存配置"],["start","启动已应用端点"],["stop","停止并释放资源"]]} onChange={v=>{setOperation(v as BatchOperation);invalidate();}} disabled={active||busy}/></label>
   <label>范围 <EnumSelect value={scope} label="批量范围" options={[["selected","勾选端点"],...(catalog?.navigation?.folders??[]).map(f=>[`folder:${f.id}`,`文件夹：${f.name}（递归）`] as const),["all","全部端点"]]} onChange={v=>{setScope(v);invalidate();}} disabled={active||busy}/></label>
   <label>并发 <input type="number" min={1} max={8} onBlur={event=>{event.currentTarget.value=String(concurrency);}} value={concurrency} disabled={active||busy} onChange={e=>{setConcurrency(Number(e.target.value));invalidate();}}/></label>
   <Button disabled={active||busy||!store.client.previewBatch} onClick={()=>void plan()}>预览清单</Button>
   {preview&&<Button disabled={busy||active} onClick={()=>void start()}>确认{preview.plan.targets.length}个端点</Button>}
   {active&&<Button disabled={busy} onClick={()=>void run(async()=>{await store.client.cancelBatch?.(view.id);setView(await store.client.queryBatch?.()??null);})}>取消后续项</Button>}
   {view?.finished&&view.counts.failed>0&&<Button disabled={busy} onClick={()=>void run(async()=>{const p=await store.client.retryBatch?.(view.id,crypto.randomUUID(),drafts());if(p){setPreview(p);setFingerprint(currentFingerprint());}})}>预览失败项重试</Button>}
  </div>
  <p className="details-hint">范围按端点固定，与点表选区无关。应用会停止实例；启动不自动应用，主站仅连接。关闭此页不取消任务。取消只影响未开始项。</p>
  {error&&<div role="alert" className="validation-banner">{error}</div>}
  {!active&&scope==="selected"&&<div className="batch-targets" aria-label="选择端点">{endpoints.map(e=><label key={e.id}><input type="checkbox" checked={selected.includes(e.id)} onChange={event=>{setSelected(ids=>event.target.checked?[...ids,e.id]:ids.filter(id=>id!==e.id));invalidate();}}/>{e.name}<small>{e.units.length} Unit</small></label>)}</div>}
  {operation==="apply"&&!active&&endpoints.filter(e=>pending(e.id)>0).map(e=><label className="batch-draft" key={e.id}><input type="checkbox" checked={discard.includes(e.id)} onChange={event=>{setDiscard(ids=>event.target.checked?[...ids,e.id]:ids.filter(id=>id!==e.id));invalidate();}}/>应用时丢弃「{e.name}」的{pending(e.id)}个待写草稿（未勾选则跳过此端点）</label>)}
  {preview&&<><h3>固定预览 · {preview.plan.targets.length}个端点</h3><div className="batch-table-wrap"><table className="batch-table"><thead><tr><th>端点</th><th>角色／Unit</th><th>当前状态</th><th>处置</th></tr></thead><tbody>{preview.plan.targets.map(t=>{const i=preview.items.find(i=>i.endpoint===t.endpoint);return <tr key={t.endpoint}><td>{t.name}</td><td>{t.role}／{t.units}</td><td>{i?.state}</td><td>{i?.reason??(preview.plan.operation==="apply"?`停止并应用 · 初始化 ${i?.changes?.[0]??0}／删除 ${i?.changes?.[1]??0}／倍率变化 ${i?.changes?.[2]??0}`:"执行前复核实际状态")}</td></tr>;})}</tbody></table></div></>}
  {view&&<><div className="batch-counts">{Object.entries(view.counts).map(([key,value])=><span key={key}>{labels[key]} {value}</span>)}</div>{view.counts.uncertain>0&&<p className="validation-banner">实际操作尚未确认结束，仍占用端点与执行槽；禁止重复提交。</p>}<div className="batch-table-wrap"><table className="batch-table"><thead><tr><th>端点</th><th>状态</th><th>排队 ms</th><th>执行 ms</th><th>原因</th></tr></thead><tbody>{view.items.map(i=><tr key={i.endpoint}><td>{endpoints.find(e=>e.id===i.endpoint)?.name??i.endpoint}</td><td>{labels[i.state]??i.state}</td><td>{i.waitMs??"—"}</td><td>{i.executionMs??"—"}</td><td>{i.reason??"—"}</td></tr>)}</tbody></table></div></>}
 </section>;
}
