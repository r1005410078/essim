import {MessagePopover} from "./MessagePopover";
import {WorkspaceIllustration} from "./WorkspaceIllustration";
import {useEffect,useMemo,useRef,useState} from "react";
import type {IDockviewPanelProps} from "dockview-react";
import {Pause,Play,Download} from "lucide-react";
import {Button} from "@essim/ui/components/button";
import {EnumSelect} from "./EnumSelect";
import {WorkspaceStore,errorMessage} from "./model";
import {mergeObservationWindow,observationRows,type ObservationWindow} from "./runtimeObservations";

/** 可见面板按需查询最近可靠观测；暂停只冻结展示，不停止通信或文件记录。 */
export function ObservationPanel({store,endpointId,kind,api,onReference}:{onReference:(anchor:string)=>void;store:WorkspaceStore;endpointId?:string;kind:string;api:IDockviewPanelProps["api"]}){
 const [visible,setVisible]=useState(api.isVisible);
 const [chosen,setChosen]=useState("");const [paused,setPaused]=useState(false);const [direction,setDirection]=useState("all");
 const [window,setWindow]=useState<ObservationWindow>({run:undefined,records:[]});const records=window.records;const [selected,setSelected]=useState("");
 const [error,setError]=useState("");const [exporting,setExporting]=useState(false);
 const endpoints=store.view?.catalog.endpoints??[];
 const followedEndpoint=useRef(endpointId);if(endpointId)followedEndpoint.current=endpointId;
 const id=chosen||(endpoints.some(endpoint=>endpoint.id===followedEndpoint.current)?followedEndpoint.current:endpoints[0]?.id);
 const master=endpoints.find(e=>e.id===id)?.role==="master";
 useEffect(()=>{const handle=api.onDidVisibilityChange(event=>setVisible(event.isVisible));return()=>handle.dispose();},[api]);
 useEffect(()=>{setWindow({run:undefined,records:[]});setSelected("");setError("");},[id]);
 useEffect(()=>{
  if(!visible||paused||!id||!store.client.querySlave)return;
  let cancelled=false;let timer:ReturnType<typeof setTimeout>;
  const poll=async()=>{try{
   const view=master?await store.client.queryMaster!({endpoint:id,unit:null,points:[]}):await store.client.querySlave!({endpoint:id,unit:null,points:[]});
   if(!cancelled){setWindow(old=>mergeObservationWindow(old,view.recordPath,view.observations??[]));setError(view.error??"");}
  }catch(e){if(!cancelled)setError(errorMessage(e));}
  finally{if(!cancelled)timer=setTimeout(poll,500);}};
  void poll();return()=>{cancelled=true;clearTimeout(timer);};
 },[visible,paused,id,store,master]);
 const rows=useMemo(()=>observationRows(records,master).filter(row=>kind==="messages"?row.direction!=="事件":row.direction==="事件").filter(row=>direction==="all"||row.direction===direction),[records,kind,direction,master]);
 const detail=rows.find(row=>row.id===selected);
 return <div className="panel column observation-panel">
  <div className="observation-tools">
   <EnumSelect label="观测端点" value={chosen} onChange={value=>{setChosen(value);setPaused(false);}} options={[["","跟随当前端点"],...endpoints.map(endpoint=>[endpoint.id,endpoint.name] as const)]}/>
   {kind==="messages"&&<EnumSelect label="报文方向" value={direction} onChange={setDirection} options={[["all","全部方向"],["RX","RX 接收"],["TX","TX 发送"]]}/>}
   <Button size="xs" variant="ghost" onClick={()=>setPaused(!paused)}>{paused?<Play size={13}/>:<Pause size={13}/>} {paused?"继续展示":"暂停展示"}</Button>
   <span className="spacer"/><span className="muted">{rows.length} 条 · 最近窗口</span>
   <Button size="xs" variant="ghost" disabled={!rows.length||exporting} onClick={async()=>{setExporting(true);try{
    if(!store.client.exportObservations)throw Error("导出服务未连接");
    await store.client.exportObservations(JSON.stringify({scope:"当前筛选视图，非完整运行记录",endpoint:id,kind,rows},null,2));
   }catch(e){setError(errorMessage(e));}finally{setExporting(false);}}}><Download size={13}/>导出</Button>
  </div>
  {error&&<div className="panel-note error" role="alert">{error}</div>}
  <div className="observation-table" role="table" aria-label={kind==="messages"?"真实通信报文":"真实运行事件"}>
   <div className="observation-row observation-heading" role="row"><span>时间</span><span>{kind==="messages"?"方向":"序号"}</span><span>连接</span><span>内容</span><span>结果</span></div>
   {rows.length?rows.map(row=><div className={`observation-row ${selected===row.id?"selected":""}`} role="row" tabIndex={0} key={row.id} onClick={()=>setSelected(row.id)} onKeyDown={event=>{if(event.target===event.currentTarget&&(event.key==="Enter"||event.key===" ")){event.preventDefault();setSelected(row.id);}}}><span>{row.time}</span><span className={`direction-${row.direction}`}>{kind==="messages"?row.direction:row.id}</span><span>{row.client}</span><span className="message-content">{kind==="messages"&&row.hex?<MessagePopover transport={row.transport} master={master} onDetails={onReference} hex={row.hex} direction={row.direction as "RX"|"TX"} requestHex={row.requestHex}/>:row.summary}</span><span>{row.status}</span></div>):<div className="observation-empty"><WorkspaceIllustration kind="observations"/><span>{id?paused?"展示已暂停":"当前窗口暂无记录，等待真实通信":"请选择或新建可运行端点"}</span></div>}
  </div>
  {detail&&<div className="observation-detail"><strong>{detail.direction} · #{detail.id}</strong> {detail.hex&&detail.direction!=="事件"?<MessagePopover transport={detail.transport} master={master} onDetails={onReference} compact hex={detail.hex} direction={detail.direction} requestHex={detail.requestHex}/>:detail.summary}{detail.direction==="TX"&&<span className="muted"> · {detail.status}，不代表远端设备动作完成</span>}</div>}
  <div className="observation-hint">每次查询最近 100 条，界面最多保留 500 条；暂停或高频通信可能跳过记录，完整记录以运行文件为准。</div>
 </div>;
}
