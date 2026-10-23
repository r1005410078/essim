import { WorkbenchDialog, DialogBody, DialogFooter } from "@essim/ui/components/workbench-dialog";
import {useEffect,useRef,useState,useSyncExternalStore} from "react";
import {Button} from "@essim/ui/components/button";
import {Plug,Unplug,Play,Square,Send,BookOpen,PauseCircle,PlayCircle,Settings2,Stethoscope,ArrowDownUp} from "lucide-react";
import {ToolbarIcon} from "./ToolbarIcon";
import {RuntimeToolbar,RuntimeHint,RuntimeError,type RuntimeToolbarSlots} from "./RuntimeToolbar";
import {ActionMenu} from "./ActionMenu";
import {EnumSelect} from "./EnumSelect";
import {resetRangePoints} from "./slaveQueryPoints";
import {acknowledgeMasterWrites,masterRaw} from "./masterWrites";
import {SheetViews,type PendingRuntimeEdit} from "./sheets";
import {WorkspaceStore,errorMessage} from "./model";
import type {CollectionModel,EndpointConfig,UnitConfig,MasterAction,MasterPoint,MasterView,MasterSettings} from "../../shared/api/contracts";
export const defaultMasterSettings:MasterSettings={connectTimeoutMs:3000,requestTimeoutMs:1000,reconnectMs:3000,staleMultiplier:3};
/** 主站所有者持续运行；面板仅按可见行查询，不以选区控制采集或下发范围。 */
export function MasterRuntimeControls({visible,store,sheets,endpoint,unit,model,readVisiblePoints,onPoints,toolbar}:{toolbar?:RuntimeToolbarSlots;visible:boolean;store:WorkspaceStore;sheets:SheetViews;endpoint:EndpointConfig;unit?:UnitConfig;model?:CollectionModel;readVisiblePoints:()=>string[];onPoints:(unit:string,points:Record<string,MasterPoint>)=>void}){
 const [view,setView]=useState<MasterView>();const [sending,setSending]=useState(false);const [error,setError]=useState("");const [settings,setSettings]=useState(false);const [range,setRange]=useState(false);const [diagnosing,setDiagnosing]=useState(false);
 const latest=useRef({unit,model,readVisiblePoints,onPoints});latest.current={unit,model,readVisiblePoints,onPoints};
 const generation=useRef(0);const pending=useRef(false);const sent=useRef<{id:string;unit:string;edits:PendingRuntimeEdit[]}|undefined>(undefined);
 useSyncExternalStore(sheets.subscribe,sheets.snapshot);
 const active=endpoint.role==="master";
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
 const rtu=endpoint.transport.kind==="rtu";
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
  <RuntimeToolbar label={rtu?"RTU主站运行":"TCP主站运行"} slots={toolbar}
   actions={<>
    <ToolbarIcon label={sessionActive?(rtu?"关闭串口":"断开连接"):(!store.view?.catalog.applied.some(e=>e.id===endpoint.id)?"连接主站：请先保存并应用配置":rtu?"使用已应用快照打开串口":"使用已应用快照连接主站")} className={sessionActive?"runtime-stop":"runtime-start"} disabled={sending||store.busy||(!sessionActive&&!!view?.busy)||!store.view?.catalog.applied.some(e=>e.id===endpoint.id)} onClick={()=>void perform({kind:sessionActive?"disconnect":"connect"})}>{sessionActive?<Unplug size={15}/>:<Plug size={15}/>}</ToolbarIcon>
    <ToolbarIcon label={view?.polling?"停止周期采集":"开始周期采集"} className={view?.polling?"runtime-stop":"runtime-start"} disabled={sending||(!view?.polling&&(busy||!connected))} onClick={()=>void perform({kind:view?.polling?"stopPolling":"startPolling"})}>{view?.polling?<Square size={15}/>:<Play size={15}/>}</ToolbarIcon>
    <ActionMenu label="手动读取" disabled={!!busy||!connected||!unit||!model} items={[
     {label:"读取当前点",disabled:!unit||!store.runtimePoint.get(unit.id),action:()=>unit&&void perform({kind:"read",unit:unit.id,points:[store.runtimePoint.get(unit.id)!]})},
     {label:"读取当前 Unit 全部点",action:()=>unit&&void perform({kind:"read",unit:unit.id,points:[]})},
     {label:"指定地址范围…",action:()=>setRange(true)},
    ]}><BookOpen size={15}/></ActionMenu>
    <ToolbarIcon label={`下发当前 Unit 修改（${edits.length}）：协议确认后自动回验，写入不自动重发`} disabled={!!busy||!connected||!edits.length} onClick={()=>void submit()}><Send size={15}/>{edits.length>0&&<span className="runtime-count">{edits.length}</span>}</ToolbarIcon>
    <ToolbarIcon label={unit&&view?.pausedUnits.includes(unit.id)?"恢复当前 Unit 采集":"暂停当前 Unit 采集"} disabled={!!busy||!unit} onClick={()=>unit&&void perform({kind:"setUnitPolling",unit:unit.id,enabled:!!view?.pausedUnits.includes(unit.id)})}>{unit&&view?.pausedUnits.includes(unit.id)?<PlayCircle size={15}/>:<PauseCircle size={15}/>}</ToolbarIcon>
   </>}
   state={<>
    <RuntimeHint label={labels[view?.state??"disconnected"]} className={`runtime-state ${connected?"running":""} ${view?.state==="failed"?"failed":""}`}><span className="status-dot"/>{labels[view?.state??"disconnected"]}</RuntimeHint>
    <RuntimeHint label={view?.polling?"周期采集中":"周期采集已停止"} className="runtime-polling">{view?.polling?"采集中":"采集停止"}</RuntimeHint>
   </>}
   metrics={<div className="runtime-metrics"><RuntimeHint label={`请求 ${view?.requests??0}`}><ArrowDownUp size={13}/>{view?.requests??0}</RuntimeHint>{view?.busy&&<RuntimeHint label={`执行进度 ${view.completed}/${view.total}`}>{view.completed}/{view.total}</RuntimeHint>}</div>}
   settings={<>
    {rtu&&<ToolbarIcon label="串行诊断" disabled={!!busy||!connected||!unit} onClick={()=>setDiagnosing(true)}><Stethoscope size={15}/></ToolbarIcon>}
    <ToolbarIcon label="主站设置（保存并应用后生效）" onClick={()=>setSettings(true)}><Settings2 size={15}/></ToolbarIcon>
   </>}/>
  <RuntimeError error={error} runtimeError={view?.error}/>
  {view?.diagnostic&&<div className="panel-note">最近诊断实际响应：<code>{view.diagnostic}</code></div>}
  {diagnosing&&unit&&<DiagnosticDialog close={()=>setDiagnosing(false)} run={(functionCode,sub,data)=>{setDiagnosing(false);void perform({kind:"diagnose",unit:unit.id,function:functionCode,sub,data});}}/>}
  {settings&&<MasterSettingsDialog rtu={rtu} initial={endpoint.master??defaultMasterSettings} close={()=>setSettings(false)} save={master=>{store.edit({...endpoint,master});setSettings(false);}}/>}
  {range&&unit&&model&&<ReadRange model={model} close={()=>setRange(false)} read={points=>{setRange(false);void perform({kind:"read",unit:unit.id,points});}}/>}
 </>;
}
function MasterSettingsDialog({initial,save,close,rtu}:{rtu:boolean;initial:MasterSettings;save:(s:MasterSettings)=>void;close:()=>void}){
 const ref=useRef<HTMLDialogElement>(null);const [value,setValue]=useState(initial);
 const fields:[keyof MasterSettings,string,number,number][]=[["connectTimeoutMs","连接超时 ms",10,30000],["requestTimeoutMs","请求超时 ms",10,30000],["reconnectMs","重连间隔 ms",100,60000],["staleMultiplier","过期周期倍数",1,100]];
 return <WorkbenchDialog ref={ref} onCancel={close} aria-label={rtu?"RTU主站设置":"TCP主站设置"} title={<>{rtu?"RTU":"TCP"} 主站设置</>}><form onSubmit={e=>{e.preventDefault();save(value);}}><DialogBody><p>修改后需保存并显式应用；应用会断开连接。</p>{fields.filter(([key])=>!rtu||key!=="connectTimeoutMs").map(([key,label,min,max])=><label key={key}>{label}<input aria-label={label} type="number" required min={min} max={max} step={1} value={value[key]} onChange={e=>setValue({...value,[key]:Number(e.target.value)})}/></label>)}</DialogBody><DialogFooter><Button type="button" variant="ghost" onClick={close}>取消</Button><Button type="submit">写入配置草稿</Button></DialogFooter></form></WorkbenchDialog>;
}
function ReadRange({model,close,read}:{model:CollectionModel;close:()=>void;read:(points:string[])=>void}){
 const ref=useRef<HTMLDialogElement>(null);const [area,setArea]=useState("holding");const [start,setStart]=useState(0);const [end,setEnd]=useState(0);const [error,setError]=useState("");
 return <WorkbenchDialog ref={ref} onCancel={close} aria-label="主站读取地址范围" title={<>读取完整点范围</>}><form onSubmit={e=>{e.preventDefault();try{const points=resetRangePoints(model.points,area,start,end);read(points);}catch(e){setError(errorMessage(e));}}}><DialogBody><p>选择范围内的完整点，不跨地址空洞合并请求。</p><EnumSelect label="读取数据区" value={area} onChange={setArea} options={[["holding","holding"],["input","input"],["coil","coil"],["discrete","discrete"]]}/><label>起始零基地址<input type="number" required min={0} max={65535} value={start} onChange={e=>setStart(Number(e.target.value))}/></label><label>结束零基地址<input type="number" required min={0} max={65535} value={end} onChange={e=>setEnd(Number(e.target.value))}/></label>{error&&<p className="error">{error}</p>}</DialogBody><DialogFooter><Button type="button" variant="ghost" onClick={close}>取消</Button><Button type="submit">读取</Button></DialogFooter></form></WorkbenchDialog>;
}

function DiagnosticDialog({close,run}:{close:()=>void;run:(fc:number,sub:number,data:number)=>void}){
 const ref=useRef<HTMLDialogElement>(null);const [fc,setFc]=useState("7");const [sub,setSub]=useState("0");const [data,setData]=useState(0);
 return <WorkbenchDialog ref={ref} onCancel={close} aria-label="RTU手动诊断" title={<>串行手动诊断</>}><form onSubmit={e=>{e.preventDefault();run(Number(fc),fc==="8"?Number(sub):0,fc==="8"&&sub==="0"?data:0);}}><DialogBody><p>使用当前Unit站号，只读查询或回送；不支持的操作将显示远端异常。</p><EnumSelect label="诊断功能" value={fc} onChange={setFc} options={[["7","07 读取异常状态"],["8","08 诊断"],["11","0B 通信事件计数"],["12","0C 通信事件日志"],["17","11 服务器标识"]]}/>{fc==="8"&&<EnumSelect label="诊断子功能" value={sub} onChange={setSub} options={[["0","0000 回送测试"],["2","0002 诊断寄存器"],...[11,12,13,14,15,16,17,18].map(n=>[String(n),`00${n.toString(16).toUpperCase().padStart(2,"0")} 通信计数`] as const)]}/>} {fc==="8"&&sub==="0"&&<label>回送数据（u16）<input type="number" required min={0} max={65535} value={data} onChange={e=>setData(Number(e.target.value))}/></label>}</DialogBody><DialogFooter><Button type="button" variant="ghost" onClick={close}>取消</Button><Button type="submit">执行诊断</Button></DialogFooter></form></WorkbenchDialog>;
}
