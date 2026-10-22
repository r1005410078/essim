import { useSyncExternalStore } from "react";
import { createRoot } from "react-dom/client";
import { FileCheck2, SlidersHorizontal } from "lucide-react";
import { TooltipProvider } from "@essim/ui/components/tooltip";
import { UnitSelector } from "../features/workspace/UnitSelector";
import { UnitPointEditor } from "../features/workspace/UnitPointEditor";
import { ToolbarIcon } from "../features/workspace/ToolbarIcon";
import { WorkspaceStore } from "../features/workspace/model";
import type { WorkspaceView, WorkspaceClient } from "../shared/api/contracts";
import "../styles/globals.css";

// 独立开发入口，仅使用明确的配置样例检查真实组件；不连接或模拟协议运行。
const view:WorkspaceView={sequence:"1",catalog:{models:[{id:"model",name:"电池簇点表",points:Array.from({length:24},(_,i)=>({id:`p${i}`,name:["总电压","总电流","SOC","SOH"][i%4]+` ${i+1}`,category:"measurement",area:"holding",address:i,valueType:"u16",scale:"1",unit:"",initial:{kind:"unsigned",value:"0"},initialEngineering:"0",byteOrder:"big",wordOrder:"big",note:"",slaveWritable:false,masterPeriodMs:1000}))}],endpoints:[{id:"endpoint",name:"工具栏预览",role:"slave",transport:{kind:"tcp",address:"127.0.0.1",port:1502},units:[{id:"a",station:1,name:"Unit 1",enabled:true,model:{id:"model"},device:null},{id:"b",station:2,name:"电池簇 B",enabled:true,model:{id:"model"},device:null},{id:"c",station:3,name:"电池簇 C",enabled:false,model:{id:"model"},device:null}]}],devices:[],applied:[],values:{}},capabilities:{configuration:false,execution:false,recording:false,replay:false,sessionOnly:true,reason:"独立组件预览"}};
const unavailable=async()=>{throw Error("组件预览不连接后端，请在正式工作台执行此操作");};
const client:WorkspaceClient={query:async()=>structuredClone(view),execute:unavailable,subscribe:async()=>async()=>{},previewPoints:async()=>({model:null,errors:[],previews:[]})};
const store=new WorkspaceStore(client);store.receive(view);
function Preview(){
 useSyncExternalStore(store.subscribe,store.snapshot);
 const unit=store.selectedUnits.get("endpoint")??"a";
 return <TooltipProvider delayDuration={250}><div style={{height:"100vh",display:"flex",flexDirection:"column"}}>
 <div style={{padding:"8px 12px",fontSize:11,color:"#77859b",background:"#f0f3f7"}}>点表真实组件预览 · 示例配置 · 无协议后端 · 可测试 Unit 搜索、编辑、菜单和 hover</div>
 <UnitPointEditor store={store} endpoint="endpoint" unit={unit} toolbar={{start:<><UnitSelector units={view.catalog.endpoints[0]!.units} value={unit} onChange={id=>{store.flushUnitEditors.get(unit)?.();store.selectUnit("endpoint",id);}}/><i className="toolbar-divider"/></>,afterSave:<><ToolbarIcon label="应用配置（预览未连接后端）" disabled><FileCheck2 size={16}/></ToolbarIcon><span className="configuration-badge">未应用</span></>,context:<span className="point-toolbar-address">127.0.0.1:1502</span>,end:<><ToolbarIcon label="配置视图"><SlidersHorizontal size={16}/></ToolbarIcon></>,menu:[{label:"保存端点配置",action:()=>void unavailable().catch(e=>{store.error=String(e);store.changed();})}]}}/>
 </div></TooltipProvider>;
}
createRoot(document.getElementById("root")!).render(<Preview/>);
