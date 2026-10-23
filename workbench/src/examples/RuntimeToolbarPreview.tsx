import { useState, useSyncExternalStore } from "react";
import { createRoot } from "react-dom/client";
import { Save, FileCheck2, ListStart, SlidersHorizontal, Activity } from "lucide-react";
import { TooltipProvider } from "@essim/ui/components/tooltip";
import { SlaveRuntimeControls } from "../features/workspace/SlaveRuntimeControls";
import { MasterRuntimeControls } from "../features/workspace/MasterRuntimeControls";
import { ConfigurationStatus } from "../features/workspace/RuntimeToolbar";
import { ToolbarIcon } from "../features/workspace/ToolbarIcon";
import { UnitSelector } from "../features/workspace/UnitSelector";
import { SheetViews } from "../features/workspace/sheets";
import { WorkspaceStore } from "../features/workspace/model";
import type { EndpointConfig, MasterView, SlaveView, WorkspaceClient, WorkspaceView } from "../shared/api/contracts";
import "../styles/globals.css";

// 独立 UI 验证夹具：状态和命令响应均为样例，不连接协议后端，不进入产品入口。
type Scenario = "未应用" | "待应用" | "运行中" | "错误" | "等待录制";
function Fixture({role,rtu,scenario}:{role:"master"|"slave";rtu:boolean;scenario:Scenario}) {
 const [log,setLog]=useState<string[]>([]);
 const [fixture]=useState(()=>{
  const unit={id:"unit",station:1,name:"Unit 1",enabled:true,model:{id:"model"},device:null};
  const endpoint:EndpointConfig={id:`${rtu?"rtu":"tcp"}-${role}`,name:"组件样例",role,transport:rtu?{kind:"rtu",path:"/dev/tty.example",baud:9600,dataBits:8,stopBits:1,parity:"none"}:{kind:"tcp",address:"127.0.0.1",port:1502},units:[unit,{...unit,id:"other",station:2,name:"电池簇 B"}]};
  const model={id:"model",name:"电池簇点表",points:[{id:"point",name:"样例点",category:role==="slave"?"measurement" as const:"control" as const,area:"holding" as const,address:0,valueType:"u16" as const,scale:"1",unit:"",initial:{kind:"unsigned" as const,value:"0"},initialEngineering:"0",byteOrder:"big" as const,wordOrder:"big" as const,note:"",slaveWritable:false,masterPeriodMs:1000}]};
  const running=scenario==="运行中"||scenario==="等待录制"||scenario==="错误";
  let slave:SlaveView={endpoint:endpoint.id,state:running?"running":"idle",revision:"1",clients:2,maxClients:10,requests:"128",exceptions:"0",lastRequestMicros:"42",error:scenario==="错误"?"录制写入失败：磁盘空间不足（UI 错误样例）":null,recordPath:null,lastReliable:"1",uncertain:scenario==="错误",pendingRecording:scenario==="等待录制",events:[],values:{}};
  let master:MasterView={endpoint:endpoint.id,state:running?"connected":"disconnected",polling:running,pausedUnits:[],busy:false,operation:null,completed:0,total:0,requests:128,error:scenario==="错误"?"请求超时：设备无响应（UI 错误样例）":null,revision:"1",points:{},observations:[],recordPath:null};
  const view:WorkspaceView={sequence:"1",catalog:{models:[model],endpoints:[endpoint],devices:[],applied:scenario==="未应用"?[]:[endpoint],values:{}},applicationMatches:{[endpoint.id]:{unit:scenario!=="未应用"&&scenario!=="待应用"}},capabilities:{configuration:false,execution:false,recording:false,replay:false,sessionOnly:true,reason:"UI 样例，无协议后端"}};
  const unavailable=async()=>{throw Error("UI 样例不提供保存和应用");};
  const client:WorkspaceClient={query:async()=>view,execute:unavailable,subscribe:async()=>async()=>{},querySlave:async()=>slave,queryMaster:async()=>master,
   executeSlave:async({action})=>{setLog(old=>[...old,action.kind]);slave={...slave,revision:String(Number(slave.revision)+1),state:action.kind==="start"?"running":action.kind==="stop"?"stopped":slave.state};return slave;},
   executeMaster:async({action})=>{setLog(old=>[...old,action.kind]);master={...master,revision:String(Number(master.revision)+1)};if(action.kind==="connect")master.state="connected";if(action.kind==="disconnect"){master.state="disconnected";master.polling=false;}if(action.kind==="startPolling")master.polling=true;if(action.kind==="stopPolling")master.polling=false;if(action.kind==="setUnitPolling")master.pausedUnits=action.enabled?[]:[action.unit];return master;}
  };
  const store=new WorkspaceStore(client);store.receive(view);const sheets=new SheetViews();
  if(running)sheets.get(unit.id,role==="slave"?"measurement":"control",model,undefined,role).updateCell(0,"draft","12");
  return {store,sheets,endpoint,unit,model};
 });
 useSyncExternalStore(fixture.store.subscribe,fixture.store.snapshot);
 const {store,sheets,endpoint,unit,model}=fixture;
 const [selected,setSelected]=useState(unit.id);
 const toolbar={start:<><UnitSelector units={endpoint.units} value={selected} onChange={setSelected}/><i className="toolbar-divider"/><ToolbarIcon label="保存端点配置（UI 样例不可用）" disabled><Save size={16}/></ToolbarIcon><ToolbarIcon label="应用配置（UI 样例不可用）" disabled><FileCheck2 size={16}/></ToolbarIcon><ConfigurationStatus dirty={false} applied={scenario!=="未应用"&&scenario!=="待应用"} hasSnapshot={scenario!=="未应用"} master={role==="master"}/></>,capture:<ToolbarIcon label="将所选确认值设为默认初始值（UI 样例）" onClick={()=>setLog(old=>[...old,"capture"])}><ListStart size={15}/></ToolbarIcon>,model:<span className="point-toolbar-model">{model.name}</span>,end:<><span className="point-toolbar-address">{rtu?"/dev/tty.example":"127.0.0.1:1502"}</span><div className="unit-view-tabs" role="tablist" aria-label="单元视图"><ToolbarIcon label="配置视图"><SlidersHorizontal size={15}/></ToolbarIcon><ToolbarIcon label="运行视图" role="tab" aria-selected><Activity size={15}/></ToolbarIcon></div></>};
 const props={visible:true,store,sheets,endpoint,unit:endpoint.units.find(u=>u.id===selected),model,readVisiblePoints:()=>[],toolbar};
 return <section style={{marginBottom:22,border:"1px solid #e3e9f1",background:"#fff"}}>
  <div style={{padding:"6px 10px",fontSize:11,color:"#8390a2"}}>{rtu?"RTU":"TCP"} {role==="slave"?"从站":"主站"}</div>
  <SlaveRuntimeControls {...props} category="measurement" onValues={()=>{}}/>
  <MasterRuntimeControls {...props} onPoints={()=>{}}/>
  <div style={{padding:"7px 10px",fontSize:10,color:"#8390a2"}}>UI 样例命令：{log.join(" → ")||"尚无操作"}</div>
 </section>;
}
function Preview(){
 const [scenario,setScenario]=useState<Scenario>("未应用");
 const [narrow,setNarrow]=useState(false);
 return <TooltipProvider delayDuration={250}><main style={{padding:16,background:"#f6f8fb",minHeight:"100vh",width:narrow?832:undefined,maxWidth:"100%"}}>
  <p style={{fontSize:12,marginBottom:12}}>运行工具栏真实组件 · 独立 UI 夹具 · 无协议后端，状态和命令响应均为样例</p>
  <div style={{display:"flex",gap:8,marginBottom:20}}>{(["未应用","待应用","运行中","错误","等待录制"] as const).map(item=><button key={item} onClick={()=>setScenario(item)} aria-pressed={scenario===item}>{item}</button>)}<button onClick={()=>setNarrow(!narrow)} aria-pressed={narrow}>800px 窄栏</button></div>
  {([false,true] as const).flatMap(rtu=>(["slave","master"] as const).map(role=><Fixture key={`${rtu}-${role}-${scenario}`} rtu={rtu} role={role} scenario={scenario}/>))}
 </main></TooltipProvider>;
}
createRoot(document.getElementById("root")!).render(<Preview/>);
