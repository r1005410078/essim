/** 独立测试壳入口；不由生产入口引用，不修改工程数据。 */
import {useState} from 'react';
import {createRoot} from 'react-dom/client';
import {armCloseGuard} from '../shared/api/window';
import {WorkspaceBoundary} from '../shared/ui/WorkspaceBoundary';
import {installExceptionHandlers,reportException,queryApplicationLog} from '../shared/api/diagnostics';
installExceptionHandlers();
void armCloseGuard(()=>false,()=>{});
function Broken():never {throw Error('diagnostic-native-react-probe');}
function Probe(){
 const [broken,setBroken]=useState(false);const[status,setStatus]=useState('');
 return <main><h1>应用诊断隔离验收</h1><p>仅测试壳入口，不打开或修改工程。</p>
 <button onClick={()=>setTimeout(()=>{throw Error('diagnostic-native-window-probe');},0)}>触发 window error</button>
 <button onClick={()=>{void Promise.reject(Error('diagnostic-native-rejection-probe'));}}>触发 unhandled rejection</button>
 <button onClick={()=>{const missing='./missing-diagnostic-entry.js';void import(/* @vite-ignore */ missing).catch(error=>reportException('entry_import',error));}}>触发入口 import 失败</button>
 <button onClick={()=>setBroken(true)}>触发 React Boundary</button>
 <button onClick={()=>void queryApplicationLog().then(value=>setStatus(JSON.stringify(value)))}>查询日志状态</button><pre>{status}</pre>
 <WorkspaceBoundary>{broken?<Broken/>:<p>渲染正常</p>}</WorkspaceBoundary></main>;
}
createRoot(document.getElementById('root')!).render(<Probe/>);
