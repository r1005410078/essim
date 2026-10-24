import type {EndpointConfig,UnitConfig,SlaveView,MasterView} from '../../shared/api/contracts';
export type TreeStatusKind='listening'|'serial'|'connected'|'polling'|'connecting'|'reconnecting'|'stopped'|'disconnected'|'failed'|'unknown'|'pending'|'dirty'|'disabled'|'paused'|'stale'|'unit-error';
export interface TreeStatus {kind:TreeStatusKind;description:string;}
export interface TreeRuntime {sequence:string;slave?:SlaveView;master?:MasterView;error?:string;}
export function treeStatuses({endpoint,unit,runtime,sequence,dirty=false,pending=false,quality,pollingEnabled=true}:{endpoint:EndpointConfig;unit?:UnitConfig;runtime?:TreeRuntime;sequence?:string;dirty?:boolean;pending?:boolean;quality?:'stale'|'failed';pollingEnabled?:boolean}):TreeStatus[]{
 const statuses:TreeStatus[]=[];
 const live=runtime?.sequence===sequence?runtime:undefined;
 if(unit){
  if(!unit.enabled)statuses.push({kind:'disabled',description:'已禁用（当前配置）'});
  else if(endpoint.role==='master'){
   if(quality)statuses.push({kind:quality==='stale'?'stale':'unit-error',description:quality==='stale'?'已观察点数据过期；仅代表最近查询的点，不是整个Unit的在线判定':'已观察点读取失败；请查看运行表中的具体错误'});
   else if(live?.master?.pausedUnits.includes(unit.id))statuses.push({kind:'paused',description:'该Unit已暂停周期采集'});
   else if(pollingEnabled&&live?.master?.state==='connected'&&live.master.polling)statuses.push({kind:'polling',description:'周期采集中；不代表所有点均读取成功'});
  }
 }else if(live?.error)statuses.push({kind:'unknown',description:`运行状态未知：${live.error}`});
 else if(endpoint.role==='slave'&&live?.slave){
  const view=live.slave;
  if(view.state==='running')statuses.push(endpoint.transport.kind==='rtu'?{kind:'serial',description:'串口应答中'}:{kind:'listening',description:`监听中 · ${view.clients} 个连接；监听不代表Unit在线`});
  else if(view.state==='failed')statuses.push({kind:'failed',description:`运行失败${view.error?`：${view.error}`:''}`});
  else if(view.state==='starting'||view.state==='stopping')statuses.push({kind:'connecting',description:view.state==='starting'?'启动中':'停止中'});
  else statuses.push({kind:'stopped',description:view.state==='idle'?'未启动':'已停止'});
 }else if(endpoint.role==='master'&&live?.master){
  const view=live.master;
  const kind=view.state==='connected'?(view.polling?'polling':'connected'):view.state==='connecting'?'connecting':view.state==='reconnecting'?'reconnecting':view.state==='failed'?'failed':'disconnected';
  const labels={polling:'周期采集中',connected:'已连接',connecting:'连接中',reconnecting:'重连等待',failed:'连接失败',disconnected:'未连接'};
  statuses.push({kind,description:`${labels[kind]}${view.error?`：${view.error}`:''}`});
 }else statuses.push({kind:'unknown',description:'运行状态尚未确认'});
 if(pending)statuses.push({kind:'pending',description:'待应用：当前配置尚未应用，运行继续使用已应用快照'});
 if(dirty)statuses.push({kind:'dirty',description:'存在未保存修改'});
 return statuses;
}
