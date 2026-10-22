import { useEffect, useRef, useState } from "react";
import { RefreshCw } from "lucide-react";
import { Button } from "@essim/ui/components/button";
import type { SerialDevice, WorkspaceClient } from "../../shared/api/contracts";
import { EnumSelect } from "./EnumSelect";

/** 发现列表与已配置路径独立；刷新、拔出或查询失败均不改写路径。 */
export function SerialPortPicker({client,value,onChange,required=false}:{client:WorkspaceClient;value:string;onChange:(value:string)=>void;required?:boolean}) {
 const [ports,setPorts]=useState<SerialDevice[]>([]);
 const [loading,setLoading]=useState(false);
 const [error,setError]=useState("");
 const alive=useRef(false);
 const generation=useRef(0);
 async function refresh() {
  const request=++generation.current;
  setLoading(true);setError("");
  try {
   if(!client.listSerialPorts)throw new Error("当前后端不支持串口发现，请手动输入路径");
   const devices=await client.listSerialPorts();
   if(alive.current && request===generation.current)setPorts(devices);
  } catch(e) {
   if(alive.current && request===generation.current)setError(typeof e==="string"?e:e instanceof Error?e.message:"串口查询失败，请重试或手动输入");
  } finally {if(alive.current && request===generation.current)setLoading(false);}
 }
 useEffect(()=>{alive.current=true;void refresh();return()=>{alive.current=false;generation.current++;};},[client]);
 const discovered=ports.some(port=>port.path===value);
 const options: [string,string][]=[ ["",loading?"正在查询串口…":"选择已发现串口"],...ports.map(port=>[port.path,`${port.name} — ${port.path}`] as [string,string]) ];
 if(value && !discovered)options.push([value,`手动路径 — ${value}`]);
 return <div className="serial-port-picker">
  <div className="serial-port-discovery">
   <EnumSelect label="已发现串口" value={value} onChange={path=>{if(path)onChange(path);}} options={options}/>
   <Button type="button" variant="ghost" size="icon" aria-label="刷新串口列表" title="刷新串口列表" disabled={loading} onClick={()=>void refresh()}><RefreshCw size={16}/></Button>
  </div>
  <input aria-label="串口路径" required={required} placeholder="选择串口或手动输入路径" value={value} onChange={event=>onChange(event.target.value)}/>
  <small className="muted" role="status">{error || (loading?"正在查询本机串口…":ports.length===0?"未发现串口，可刷新或手动输入；虚拟串口也可手动填写。":discovered?ports.find(port=>port.path===value)?.name:"可选择设备，或在下方手动输入完整路径。")}</small>
 </div>;
}
