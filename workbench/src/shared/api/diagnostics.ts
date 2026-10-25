/** 应用诊断唯一传输边界；浏览器预览仅写 Console。 */
import { invoke, isTauri } from '@tauri-apps/api/core';
export interface ApplicationLogStatus { fileAvailable:boolean; directory:string; dropped:number; writeFailures:number; lastError?:string|null }
export interface DiagnosticRecord { level:'ERROR'|'WARN'|'INFO'|'DEBUG'|'TRACE'; event:string; message:string; stack?:string }
const clip=(text:string,max:number)=>new TextDecoder().decode(new TextEncoder().encode(text).slice(0,max));
export function createDiagnosticReporter(send:(record:DiagnosticRecord)=>Promise<unknown>, now=Date.now) {
 const seen=new Map<string,number>(); let second=0,count=0,failed=false;
 return (event:string,error:unknown,stack?:string)=>{
  // 不序列化任意对象，Error 之外仅接受文本。
  const message=error instanceof Error?error.message:typeof error==='string'?error:'非文本异常';
  const trace=stack??(error instanceof Error?error.stack:undefined);
  const record:DiagnosticRecord={level:'ERROR',event:clip(event,128),message:clip(message,3000),stack:trace?clip(trace,5000):undefined};
  const time=now(),fingerprint=JSON.stringify(record);
  for(const [key,at] of seen) if(time-at>=5000)seen.delete(key);
  if(seen.has(fingerprint))return;
  if(Math.floor(time/1000)!==second){second=Math.floor(time/1000);count=0;}
  if(count>=20)return; count++;seen.set(fingerprint,time);
  // 再限制转义后的整个 IPC 载荷；控制字符可能扩大 JSON。
  while(new TextEncoder().encode(JSON.stringify(record)).length>15000){record.message=record.message.slice(0,record.message.length/2);record.stack=record.stack?.slice(0,record.stack.length/2);}
  const fallback=()=>{if(!failed){failed=true;console.warn('应用诊断提交失败，后续失败不再重复输出');}};
  try {void send(record).catch(fallback);} catch {fallback();}
 };
}
export const reportException=createDiagnosticReporter(async record=>{
 if(isTauri())await invoke('submit_application_log',{record});
 else console.error('[浏览器诊断，仅 Console]',record);
});
export function installExceptionHandlers(){
 const error=(event:ErrorEvent)=>reportException('window_error',event.error??event.message);
 const rejection=(event:PromiseRejectionEvent)=>reportException('unhandled_rejection',event.reason);
 window.addEventListener('error',error);window.addEventListener('unhandledrejection',rejection);
 return ()=>{window.removeEventListener('error',error);window.removeEventListener('unhandledrejection',rejection);};
}
export async function queryApplicationLog():Promise<ApplicationLogStatus|null>{return isTauri()?invoke('query_application_log'):null;}
export async function openApplicationLogDirectory(){if(isTauri())await invoke('open_application_log_directory');}
