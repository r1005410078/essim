import type {SlaveObservation} from "../../shared/api/contracts";
export function mergeObservations(previous:SlaveObservation[],incoming:SlaveObservation[]):SlaveObservation[]{
 const records=new Map(previous.map(row=>[row.sequence,row]));
 for(const row of incoming)records.set(row.sequence,row);
 return [...records.values()].sort((a,b)=>BigInt(a.sequence)<BigInt(b.sequence)?-1:1).slice(-500);
}
export interface ObservationRow {id:string;time:string;direction:"RX"|"TX"|"事件";client:string;summary:string;hex:string;status:string;transaction?:number;unit?:number;functionCode?:number;requestHex?:string;transport?:"tcp"|"rtu"}
const labels:Record<string,string>={listening:"监听已建立",stopping:"正在停止",stopped:"资源已释放",connected:"主站已连接",disconnected:"连接已关闭",responseSent:"响应已写出socket（字节记录已离开窗口）",valuesIntent:"运行值提交已记录",valuesCommitted:"运行值已提交"};
/** 只将同一客户端按队列顺序的完整发送确认关联到响应；窗口缺失时不猜字节。 */
export function observationRows(records:SlaveObservation[],master=false):ObservationRow[]{
 const pending=new Map<string,ObservationRow[]>();
 const requests=new Map<string,{hex:string[];ambiguous:boolean}>();
 return records.map(record=>{
  let fact:Record<string,unknown>;try{fact=JSON.parse(record.event);}catch{fact={};}
  const kind=String(fact?.kind??"");const client=String(fact?.client??"—");
  const frame=(kind==="frame"||kind==="rtuFrame")&&typeof fact.hex==="string"&&/^(?:[0-9a-f]{2} ?)+$/i.test(fact.hex)?fact.hex.split(" ").map(b=>Number.parseInt(b,16)):undefined;
  const bytes=frame??(Array.isArray(fact?.bytes)&&fact.bytes.every(b=>Number.isInteger(b)&&b>=0&&b<=255)?fact.bytes as number[]:[]);
  const date=new Date(Number(record.unixMs));
  const row:ObservationRow={id:record.sequence,time:Number.isNaN(date.valueOf())?"—":date.toLocaleTimeString("zh-CN",{hour12:false})+`.`+String(date.getMilliseconds()).padStart(3,"0"),direction:frame?(fact.direction==="TX"?"TX":"RX"):kind==="request"?"RX":kind==="responsePrepared"?"TX":"事件",client,summary:labels[kind]??"无法解析的观测记录",hex:bytes.map(b=>b.toString(16).padStart(2,"0").toUpperCase()).join(" "),status:"已记录"};
  const rtu=kind==="rtuFrame";row.transport=rtu?"rtu":"tcp";
  if(rtu&&bytes.length>=2){row.unit=bytes[0];row.functionCode=bytes[1];row.status=fact.discarded?"迟到／非在途数据已隔离":!fact.complete?"短帧／不完整":!fact.crcValid?"CRC错误":row.direction==="TX"?"发送准备（非远端确认）":"实际RTU接收字节";}
  if(!rtu&&bytes.length>=8){row.transaction=(bytes[0]!<<8)|bytes[1]!;row.unit=bytes[6];row.functionCode=bytes[7];}
  if(row.direction!=="事件"){
   row.summary=`Unit ${row.unit??"?"} · FC ${row.functionCode??"?"} · ${bytes.length} 字节`;
   row.status=rtu?row.status:frame?"实际通道字节":kind==="request"?"已接收":"待发送";
  }else if(typeof fact?.reason==="string")row.summary+=`：${fact.reason}`;
  else if(typeof fact?.peer==="string")row.summary+=` · ${fact.peer}`;
  if((rtu?bytes.length>=4:bytes.length>=8)&&(client!=="—"||!!frame)&&(!rtu||fact.complete&&fact.crcValid&&!fact.discarded)){
   const key=`${client}:${row.transaction}:${row.unit}:${(row.functionCode??0)&0x7f}`;
   if(kind==="request"||(frame&&master&&row.direction==="TX")){
    const previous=requests.get(key);requests.set(key,{hex:[...(previous?.hex??[]),row.hex],ambiguous:!!previous});
   }else if(kind==="responsePrepared"||(frame&&master&&row.direction==="RX")){
    const candidate=requests.get(key);
    if(candidate){const request=candidate.hex.shift();if(!candidate.ambiguous)row.requestHex=request;if(!candidate.hex.length)requests.delete(key);}
   }
  }
  if(kind==="state"&&["connecting","disconnected","reconnecting"].includes(String(fact.state)))requests.clear();
  if(kind==="state")row.summary=`主站状态：${String(fact.state??"")}`;
  if(kind==="failure")row.summary=String(fact.message??"主站失败");
  if(kind==="intent")row.summary="主站操作意图";
  if(kind==="operation")row.summary=String(fact.result??"操作完成");
  if(kind==="disconnected")for(const key of requests.keys())if(key.startsWith(`${client}:`))requests.delete(key);
  if(kind==="responsePrepared")pending.set(client,[...(pending.get(client)??[]),row]);
  if(kind==="responseSent"){
   const prepared=pending.get(client)?.shift();
   if(prepared){prepared.status="已写出socket";row.summary="响应已完整写出socket";}
  }
  return row;
 });
}

export interface ObservationWindow {run:string|null|undefined;records:SlaveObservation[]}
/** 每次启动的可靠序号从头开始，运行身份改变时不能把两次运行混在一起。 */
export function mergeObservationWindow(previous:ObservationWindow,run:string|null,incoming:SlaveObservation[]):ObservationWindow {
 return {run,records:mergeObservations(previous.run!==undefined&&previous.run!==run?[]:previous.records,incoming)};
}
