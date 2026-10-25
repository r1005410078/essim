import {useEffect,useState} from 'react';
import {queryApplicationLog,openApplicationLogDirectory,type ApplicationLogStatus} from '../../shared/api/diagnostics';
export function applicationLogIssue(status:ApplicationLogStatus|null):string|null {
 if(!status || (status.fileAvailable&&!status.dropped&&!status.writeFailures))return null;
 return `应用日志${status.fileAvailable?'有诊断丢弃或写入失败':'不可用'} · 丢弃 ${status.dropped} · 写入失败 ${status.writeFailures}${status.lastError?` · ${status.lastError}`:''}`;
}
/** 应用日志失败独立于校验问题，不影响工程或协议状态。 */
export function ApplicationLogProblem(){
 const [status,setStatus]=useState<ApplicationLogStatus|null>(null);
 useEffect(()=>{let active=true;const poll=()=>void queryApplicationLog().then(value=>{if(active)setStatus(value);}).catch(()=>{});poll();const timer=setInterval(poll,5000);return()=>{active=false;clearInterval(timer);};},[]);
 const issue=applicationLogIssue(status);
 return issue?<div className="panel-note error" role="status">{issue} <button onClick={()=>void openApplicationLogDirectory().catch(error=>console.warn('打开日志目录失败',error))}>打开日志目录</button></div>:null;
}
