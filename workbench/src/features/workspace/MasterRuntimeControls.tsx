import {useEffect,useRef,useState,useSyncExternalStore} from "react";
import {Button} from "@essim/ui/components/button";
import {Plug,Unplug,Play,Square,Send} from "lucide-react";
import {ActionMenu} from "./ActionMenu";
import {EnumSelect} from "./EnumSelect";
import {resetRangePoints} from "./slaveQueryPoints";
import {acknowledgeMasterWrites,masterRaw} from "./masterWrites";
import {SheetViews,type PendingRuntimeEdit} from "./sheets";
import {WorkspaceStore,errorMessage} from "./model";
import type {CollectionModel,EndpointConfig,UnitConfig,MasterAction,MasterPoint,MasterView,MasterSettings} from "../../shared/api/contracts";
export const defaultMasterSettings:MasterSettings={connectTimeoutMs:3000,requestTimeoutMs:1000,reconnectMs:3000,staleMultiplier:3};
/** 主站所有者持续运行；面板仅按可见行查询，不以选区控制采集或下发范围。 */
export function MasterRuntimeControls({visible,store,sheets,endpoint,unit,model,readVisiblePoints,onPoints}:{visible:boolean;store:WorkspaceStore;sheets:SheetViews;endpoint:EndpointConfig;unit?:UnitConfig;model?:CollectionModel;readVisiblePoints:()=>string[];onPoints:(unit:string,points:Record<string,MasterPoint>)=>void}){
 const [view,setView]=useState<MasterView>();const [sending,setSending]=useState(false);const [error,setError]=useState("");const [settings,setSettings]=useState(false);const [range,setRange]=useState(false);
 const latest=useRef({unit,model,readVisiblePoints,onPoints});latest.current={unit,model,readVisiblePoints,onPoints};
 const generation=useRef(0);const pending=useRef(false);const sent=useRef<{id:string;unit:string;edits:PendingRuntimeEdit[]}|undefined>(undefined);
 useSyncExternalStore(sheets.subscribe,sheets.snapshot);
 const active=endpoint.role==="master"&&endpoint.transport.kind==="tcp";
 const query=async()=>{
  const ticket=generation.current;const current=latest.current;const sequence=store.view?.sequence;
  const known=new Set(current.model?.points.map(p=>p.id));const ids=[...new Set(current.readVisiblePoints())].filter(id=>known.has(id)).slice(0,1024);
  const next=await store.client.queryMaster?.({endpoint:endpoint.id,unit:current.unit?.id??null,points:ids});
  if(next&&ticket===generation.current&&sequence===store.view?.sequence){setView(next);if(current.unit)current.onPoints(current.unit.id,next.points);
    if(sent.current&&next.operation===sent.current.id&&!next.busy){
      // 只清除回验一致的草稿；拒绝／不确定／未执行仍留待用户核对。
      const job=sent.current;sent.current=undefined;
      const all=await store.client.queryMaster?.({endpoint:endpoint.id,unit:job.unit,points:job.edits.map(e=>e.point.id).slice(0,1024)});
      if(all&&ticket===generation.current)acknowledgeMasterWrites(sheets,job.id,job.edits,all);
    }
  }
 };
 const queryRef=useRef(query);queryRef.current=query;
 useEffect(()=>{generation.current++;},[unit?.id,store.view?.sequence]);
 useEffect(()=>{if(!visible||!active)return;let cancelled=false;let timer:ReturnType<typeof setTimeout>;
  const poll=async()=>{try{if(!pending.current)await queryRef.current();}catch(e){if(!cancelled)setError(errorMessage(e));}finally{if(!cancelled)timer=setTimeout(poll,300);}};
  void poll();return()=>{cancelled=true;clearTimeout(timer);generation.current++;};
 },[visible,active,endpoint.id,store]);
 const perform=async(action:MasterAction,edits?:PendingRuntimeEdit[])=>{
  if(pending.current)return;pending.current=true;generation.current++;setSending(true);setError("");
  try{const commandId=crypto.randomUUID();const next=await store.client.executeMaster?.({commandId,endpoint:endpoint.id,action});if(!next)throw Error("主站服务未连接");setView(next);if(edits)sent.current={id:commandId,unit:action.kind==="write"?action.unit:"",edits};}
  catch(e){setError(errorMessage(e));}finally{pending.current=false;setSending(false);}
 };
 if(!active)return null;
 const sessionActive=!!view&&(["connected","connecting","reconnecting"].includes(view.state)||view.polling);
 const connected=view?.state==="connected";const busy=sending||view?.busy||store.busy;
 const edits=unit&&model?sheets.pendingMaster(unit.id,model):[];
 const submit=async()=>{try{
  if(document.activeElement instanceof HTMLElement)document.activeElement.blur();await Promise.resolve();if(!unit||!model)return;
  const changes=sheets.pendingMaster(unit.id,model);if(!changes.length)throw Error("当前Unit没有待写修改");if(changes.length>1024)throw Error("首版单批下发最多1024个完整点，请分批操作");
  await perform({kind:"write",unit:unit.id,values:changes.map(e=>[e.point.id,masterRaw(e.point,e.text)])},changes);
 }catch(e){setError(errorMessage(e));}};
 const labels:Record<string,string>={disconnected:"未连接",connecting:"连接中",connected:"已连接",reconnecting:"重连等待",failed:"连接失败"};
 return <>
  <div className="runtime-summary" aria-label="TCP主站运行"><span className={`runtime-state ${connected?"running":""}`}><span className="status-dot"/>{labels[view?.state??"disconnected"]}</span><span>{view?.polling?"周期采集中":"采集已停止"}</span><span>请求 {view?.requests??0}</span>{view?.busy&&<span>执行进度 {view.completed}/{view.total}</span>}<span className="spacer"/>
   <Button size="sm" variant="outline" disabled={sending||store.busy||(!sessionActive&&!!view?.busy)||!store.view?.catalog.applied.some(e=>e.id===endpoint.id)} onClick={()=>void perform({kind:sessionActive?"disconnect":"connect"})}>{sessionActive?<Unplug size={14}/>:<Plug size={14}/>} {sessionActive?"断开":"连接"}</Button>
   <Button size="sm" disabled={sending||(!view?.polling&&(busy||!connected))} onClick={()=>void perform({kind:view?.polling?"stopPolling":"startPolling"})}>{view?.polling?<Square size={14}/>:<Play size={14}/>} {view?.polling?"停止采集":"开始采集"}</Button>
  </div>
  <div className="toolbar runtime-actions"><ActionMenu label="手动读取" disabled={!!busy||!connected||!unit||!model} items={[
   {label:"读取当前点",disabled:!unit||!store.runtimePoint.get(unit.id),action:()=>unit&&void perform({kind:"read",unit:unit.id,points:[store.runtimePoint.get(unit.id)!]})},
   {label:"读取当前 Unit 全部点",action:()=>unit&&void perform({kind:"read",unit:unit.id,points:[]})},
   {label:"指定地址范围…",action:()=>setRange(true)},
  ]}/><Button size="sm" variant="ghost" disabled={!!busy||!connected||!edits.length} onClick={()=>void submit()}><Send size={14}/>下发修改（{edits.length}）</Button><Button size="sm" variant="ghost" disabled={!!busy||!unit} onClick={()=>unit&&void perform({kind:"setUnitPolling",unit:unit.id,enabled:!!view?.pausedUnits.includes(unit.id)})}>{unit&&view?.pausedUnits.includes(unit.id)?"恢复当前 Unit 采集":"暂停当前 Unit 采集"}</Button><span className="muted">协议确认后自动回验 · 写入不自动重发</span><span className="spacer"/><Button size="sm" variant="ghost" onClick={()=>setSettings(true)}>主站设置</Button></div>
  {!store.view?.applicationMatches?.[endpoint.id]||Object.values(store.view.applicationMatches[endpoint.id]??{}).some(x=>!x)||store.drafts.has(endpoint.id)?<div className="configuration-notice">存在未应用修改 · 连接和采集使用已应用快照</div>:null}
  {(error||view?.error)&&<div className="panel-note error" role="alert">{error||view?.error}</div>}
  {settings&&<MasterSettingsDialog initial={endpoint.master??defaultMasterSettings} close={()=>setSettings(false)} save={master=>{store.edit({...endpoint,master});setSettings(false);}}/>}
  {range&&unit&&model&&<ReadRange model={model} close={()=>setRange(false)} read={points=>{setRange(false);void perform({kind:"read",unit:unit.id,points});}}/>}
 </>;
}
function MasterSettingsDialog({initial,save,close}:{initial:MasterSettings;save:(s:MasterSettings)=>void;close:()=>void}){
 const ref=useRef<HTMLDialogElement>(null);const [value,setValue]=useState(initial);useEffect(()=>{ref.current?.showModal();},[]);
 const fields:[keyof MasterSettings,string,number,number][]=[["connectTimeoutMs","连接超时 ms",10,30000],["requestTimeoutMs","请求超时 ms",10,30000],["reconnectMs","重连间隔 ms",100,60000],["staleMultiplier","过期周期倍数",1,100]];
 return <dialog ref={ref} onCancel={close} aria-label="TCP主站设置"><form onSubmit={e=>{e.preventDefault();save(value);}}><h2>TCP 主站设置</h2><p>修改后需保存并显式应用；应用会断开连接。</p>{fields.map(([key,label,min,max])=><label key={key}>{label}<input aria-label={label} type="number" required min={min} max={max} step={1} value={value[key]} onChange={e=>setValue({...value,[key]:Number(e.target.value)})}/></label>)}<div className="dialog-actions"><Button type="button" variant="ghost" onClick={close}>取消</Button><Button type="submit">写入配置草稿</Button></div></form></dialog>;
}
function ReadRange({model,close,read}:{model:CollectionModel;close:()=>void;read:(points:string[])=>void}){
 const ref=useRef<HTMLDialogElement>(null);const [area,setArea]=useState("holding");const [start,setStart]=useState(0);const [end,setEnd]=useState(0);const [error,setError]=useState("");useEffect(()=>{ref.current?.showModal();},[]);
 return <dialog ref={ref} onCancel={close} aria-label="主站读取地址范围"><form onSubmit={e=>{e.preventDefault();try{const points=resetRangePoints(model.points,area,start,end);read(points);}catch(e){setError(errorMessage(e));}}}><h2>读取完整点范围</h2><p>选择范围内的完整点，不跨地址空洞合并请求。</p><EnumSelect label="读取数据区" value={area} onChange={setArea} options={[["holding","holding"],["input","input"],["coil","coil"],["discrete","discrete"]]}/><label>起始零基地址<input type="number" required min={0} max={65535} value={start} onChange={e=>setStart(Number(e.target.value))}/></label><label>结束零基地址<input type="number" required min={0} max={65535} value={end} onChange={e=>setEnd(Number(e.target.value))}/></label>{error&&<p className="error">{error}</p>}<div className="dialog-actions"><Button type="button" variant="ghost" onClick={close}>取消</Button><Button type="submit">读取</Button></div></form></dialog>;
}
