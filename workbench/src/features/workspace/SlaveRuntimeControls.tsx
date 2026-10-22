import {Button} from "@essim/ui/components/button";
import {ActionMenu} from "./ActionMenu";
import {resetRangePoints} from "./slaveQueryPoints";
import {SlaveResponseGate} from "./slaveResponses";
import {useEffect,useRef,useState,useSyncExternalStore} from "react";
import {Play,Square,Check,RefreshCw} from "lucide-react";
import type {Category,CollectionModel,EndpointConfig,RawValue,SlaveAction,SlaveView,UnitConfig} from "../../shared/api/contracts";
import {WorkspaceStore,errorMessage} from "./model";
import {SheetViews} from "./sheets";
import {slaveRaw} from "./slaveValues";
import {EnumSelect} from "./EnumSelect";

/** 查询只跟随当前可见行；提交从当前Unit的待修改草稿收集，与选区无关。 */
export function SlaveRuntimeControls({visible,store,sheets,endpoint,unit,model,category,readVisiblePoints,onValues}:{
 visible:boolean;store:WorkspaceStore;sheets:SheetViews;endpoint:EndpointConfig;unit?:UnitConfig;model?:CollectionModel;category:Category;
 readVisiblePoints:()=>string[];onValues:(unit:string,values:Record<string,RawValue>|undefined)=>void;
}){
 const [view,setView]=useState<SlaveView>();const [busy,setBusy]=useState(false);const [error,setError]=useState("");
 const [limit,setLimit]=useState(10);const [configuring,setConfiguring]=useState(false);const [reset,setReset]=useState<"unit"|"range"|null>(null);
 useSyncExternalStore(sheets.subscribe,sheets.snapshot);
 const responseGate=useRef(new SlaveResponseGate());const pending=useRef(false);
 useEffect(()=>{responseGate.current.invalidate();setReset(null);},[endpoint.id,unit?.id,category,store.view?.sequence]);
 const latest=useRef({unit,model,category,readVisiblePoints,onValues});latest.current={unit,model,category,readVisiblePoints,onValues};
 const active=endpoint.role==="slave";
 const query=async()=>{
  const ticket=responseGate.current.ticket();const sequence=store.view?.sequence;
  const current=latest.current;
  const known=new Set(current.model?.points.filter(point=>point.category===current.category).map(point=>point.id));
  const ids=[...new Set(current.readVisiblePoints())].filter(id=>known.has(id)).slice(0,1024);
  const next=await store.client.querySlave?.({endpoint:endpoint.id,unit:current.unit?.id??null,points:ids});
  if(next&&store.view?.sequence===sequence&&responseGate.current.accept(ticket,next)){
   setView(next);if(current.unit)current.onValues(current.unit.id,next.state==="idle"?undefined:next.values);
  }
  return next;
 };
 const queryLatest=useRef(query);queryLatest.current=query;
 useEffect(()=>{
  if(!visible||!active||!store.client.querySlave)return;
  let cancelled=false;let timer:ReturnType<typeof setTimeout>;
  const poll=async()=>{try{if(!pending.current&&!cancelled)await queryLatest.current();}
   catch(e){if(!cancelled)setError(errorMessage(e));}
   finally{if(!cancelled)timer=setTimeout(poll,500);}};
  void poll();return()=>{cancelled=true;clearTimeout(timer);responseGate.current.invalidate();};
 },[visible,active,endpoint.id,store]);
 const perform=async(action:SlaveAction,committed?:()=>void)=>{
  if(pending.current)return false;
  pending.current=true;responseGate.current.invalidate();const ticket=responseGate.current.ticket();setBusy(true);setError("");
  try{
   const result=await store.client.executeSlave?.({commandId:crypto.randomUUID(),endpoint:endpoint.id,action});
   if(!result)throw Error("运行服务未连接");
   committed?.();
   if(responseGate.current.accept(ticket,result))setView(result);
   await queryLatest.current();
   if(action.kind==="start"||action.kind==="stop")await store.refresh();
   return true;
  }catch(e){
   const pointId=(e as {pointId?:string})?.pointId;const point=latest.current.model?.points.find(point=>point.id===pointId);
   setError(`${point?`${point.name}（${point.area} 地址${point.address}）：`:""}${errorMessage(e)}`);
   await queryLatest.current().catch(()=>{});return false;
  }finally{pending.current=false;setBusy(false);}
 };
 if(!active)return null;
 const edits=unit&&model?sheets.pending(unit.id,model):[];
 const ready=!!view&&view.state!=="idle";
 const running=view?.state==="running"||view?.state==="starting"||view?.state==="stopping";
 const submit=async()=>{
  if(!unit||!model)return;
  try{
   // 先让NovaSheet提交编辑框文字，再收集所有分类中真实修改的点。
   if(document.activeElement instanceof HTMLElement)document.activeElement.blur();
   await Promise.resolve();
   const sent=sheets.pending(unit.id,model);
   if(!sent.length){setError("当前Unit没有待提交修改");return;}
   const values=sent.map(edit=>[edit.point.id,slaveRaw(edit.point,edit.text)] as [string,RawValue]);
   await perform({kind:"setValues",unit:unit.id,values},()=>sheets.acknowledge(sent));
  }catch(e){setError(errorMessage(e));}
 };
 const rtu=endpoint.transport.kind==="rtu";
 const labels={idle:"未启动",starting:"启动中",running:rtu?"串口应答中":"监听中",stopping:"停止中",stopped:"已停止",failed:"运行失败"};
 return <>
  <div className="runtime-summary" aria-label={rtu?"RTU从站运行":"TCP从站运行"}>
   <span className={`runtime-state ${running?"running":""}`}><span className="status-dot"/>{labels[view?.state??"idle"]}</span>
   {view?.pendingRecording&&<span className="muted">等待可靠录制 · 显示最近可靠值</span>}
   {!rtu&&<span>连接 {view?.clients??0}/{view?.maxClients??limit}</span>}<span>请求 {view?.requests??0}</span><span className={Number(view?.exceptions)>0?"error":""}>异常 {view?.exceptions??0}</span>
   <span className="runtime-timing muted">最近处理 {view?.lastRequestMicros??"—"} μs</span><span className="spacer"/>
   <Button size="sm" variant={running?"outline":"default"} disabled={busy||store.busy||!store.client.executeSlave||(!running&&!store.view?.catalog.applied.some(item=>item.id===endpoint.id))} title={running?"停止当前从站并释放资源":"使用已应用快照启动从站"} onClick={()=>void perform(running?{kind:"stop"}:{kind:"start",maxClients:limit})}>{running?<Square size={14}/>:<Play size={14}/>} {rtu?(running?"关闭串口":"打开串口"):(running?"停止":"启动")}</Button>
  </div>
  <div className="toolbar runtime-actions">
   <Button size="sm" variant="ghost" disabled={busy||!unit||!ready} title="原子提交当前Unit所有待修改点，与选区无关" onClick={()=>void submit()}><Check size={15}/>提交修改（{edits.length}）</Button>
   <ActionMenu label="恢复初值" disabled={busy||!unit||!ready} items={[{label:"指定地址范围…",action:()=>setReset("range")},{label:"复位当前 Unit…",action:()=>setReset("unit")}]}/>
   <Button size="sm" variant="ghost" disabled={busy} title="刷新当前可见行的实际值" aria-label="刷新可见行" onClick={()=>void query().catch(e=>setError(errorMessage(e)))}><RefreshCw size={14}/></Button>
   <span className="spacer"/>
   {!rtu&&<ActionMenu label="运行设置" items={[{label:`连接上限 ${limit}…`,disabled:!!running||busy,action:()=>setConfiguring(true)}]}/>}
  </div>
  {(store.drafts.has(endpoint.id)||!store.view?.applicationMatches?.[endpoint.id]||Object.values(store.view.applicationMatches[endpoint.id]??{}).some(value=>!value))&&<div className="configuration-notice">配置尚未应用 · 启动使用已应用快照，请先保存并应用配置。</div>}
  {(error||view?.error)&&<div className="panel-note error" role="alert">{error||view?.error}{view?.uncertain?"；存在未确认结果，请勿据此重复写入。":""}</div>}
  {configuring&&<ConnectionSettings limit={limit} close={()=>setConfiguring(false)} save={value=>{setLimit(value);setConfiguring(false);}}/>}
  {reset==="range"&&unit&&model&&<RangeReset model={model} busy={busy} cancel={()=>setReset(null)} confirm={async points=>{if(await perform({kind:"resetValues",unit:unit.id,points}))setReset(null);}}/>}
  {reset==="unit"&&unit&&<ResetConfirmation name={unit.name} busy={busy} cancel={()=>setReset(null)} confirm={()=>{setReset(null);void perform({kind:"resetValues",unit:unit.id,points:[]});}}/>}
 </>;
}

function RangeReset({model,busy,cancel,confirm}:{model:CollectionModel;busy:boolean;cancel:()=>void;confirm:(points:string[])=>Promise<void>}){
 const dialog=useRef<HTMLDialogElement>(null);const [area,setArea]=useState("holding");const [start,setStart]=useState("");const [end,setEnd]=useState("");const [error,setError]=useState("");
 useEffect(()=>{dialog.current?.showModal();},[]);
 return <dialog ref={dialog} aria-label="恢复地址范围初值" onCancel={event=>{event.preventDefault();if(!busy)cancel();}}>
  <form onSubmit={event=>{event.preventDefault();try{
   if(!/^\d+$/.test(start)||!/^\d+$/.test(end))throw Error("请填写起止零基地址");
   const ids=resetRangePoints(model.points,area,Number(start),Number(end));setError("");void confirm(ids);
  }catch(e){setError(errorMessage(e));}}}>
   <h2>恢复地址范围初值</h2><p>仅恢复当前Unit已应用模型中指定数据区的完整点；保留未提交草稿。</p>
   <label>数据区<EnumSelect label="恢复数据区" value={area} onChange={setArea} options={[["holding","holding"],["input","input"],["coil","coil"],["discrete","discrete"]]}/></label>
   <label>起始零基地址<input aria-label="恢复起始地址" required type="number" min={0} max={65535} value={start} onChange={e=>setStart(e.target.value)}/></label>
   <label>结束零基地址<input aria-label="恢复结束地址" required type="number" min={0} max={65535} value={end} onChange={e=>setEnd(e.target.value)}/></label>
   {error&&<p className="error">{error}</p>}
   <div className="dialog-actions"><button type="button" disabled={busy} onClick={cancel}>取消</button><button type="submit" disabled={busy}>恢复范围初值</button></div>
  </form>
 </dialog>;
}
function ResetConfirmation({name,busy,cancel,confirm}:{name:string;busy:boolean;cancel:()=>void;confirm:()=>void}) {
 const dialog=useRef<HTMLDialogElement>(null);
 useEffect(()=>{dialog.current?.showModal();},[]);
 return <dialog ref={dialog} aria-label="复位当前Unit" onCancel={event=>{event.preventDefault();if(!busy)cancel();}}>
  <h2>复位当前Unit</h2><p>将 {name} 全部点恢复为已应用模型初始值，不影响其他Unit。</p>
  <div className="dialog-actions"><button disabled={busy} onClick={cancel}>取消</button><button disabled={busy} onClick={confirm}>确认复位</button></div>
 </dialog>;
}

function ConnectionSettings({limit,close,save}:{limit:number;close:()=>void;save:(value:number)=>void}){
 const dialog=useRef<HTMLDialogElement>(null);const [value,setValue]=useState(String(limit));
 useEffect(()=>{dialog.current?.showModal();},[]);
 return <dialog ref={dialog} aria-label="TCP运行设置" onCancel={close}><form onSubmit={event=>{event.preventDefault();const next=Number(value);if(Number.isInteger(next)&&next>=1&&next<=100)save(next);}}><h2>TCP 运行设置</h2><p>仅用于下一次启动，当前会话有效。</p><label>主站连接上限<input aria-label="TCP主站连接上限" type="number" required min={1} max={100} value={value} onChange={event=>setValue(event.target.value)}/></label><div className="dialog-actions"><Button type="button" variant="ghost" onClick={close}>取消</Button><Button type="submit">保存设置</Button></div></form></dialog>;
}
