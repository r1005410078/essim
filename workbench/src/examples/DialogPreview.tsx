import { useState } from 'react';
import { createRoot } from 'react-dom/client';
import { WorkbenchDialog, DialogBody, DialogFooter } from '@essim/ui/components/workbench-dialog';
import { EnumSelect } from '../features/workspace/EnumSelect';
import '../styles/globals.css';
// 仅用于交互验证；不接后端，不进入产品入口。
function Preview(){
 const [open,setOpen]=useState(false),[busy,setBusy]=useState(false),[long,setLong]=useState(false),[value,setValue]=useState('tcp'),[result,setResult]=useState('');
 return <main style={{padding:24,background:'#f3f4f6',minHeight:'100vh',fontSize:12}}><button onClick={()=>setOpen(true)}>打开弹框</button><p>提交结果：{result||'尚无'}</p>{open&&<WorkbenchDialog title="新建模拟器" busy={busy} onCancel={()=>setOpen(false)}><form onSubmit={event=>{event.preventDefault();setResult(new FormData(event.currentTarget).get('name') as string);setOpen(false);}}><DialogBody>
  <label>名称<input autoFocus name="name" aria-label="名称" required/></label><label>首个 Unit 名称<input defaultValue="Unit 1"/></label><label>站号<input type="number" min={1} max={247} defaultValue={1} required/></label><label>采集模型<input placeholder="搜索模型，或输入新模型名称"/></label><label>通信角色<EnumSelect label="通信角色" value="slave" onChange={()=>{}} options={[["slave","从站"],["master","主站"]]}/></label><label>传输<EnumSelect label="传输" value={value} onChange={setValue} options={[["tcp","TCP"],["rtu","RTU"]]}/></label><label>IPv4 地址<input defaultValue="127.0.0.1"/></label><label>端口<input type="number" defaultValue={1502}/></label>
  <p className="muted">保存到本次会话，不启动通信；退出后不保留。</p><div style={{display:'flex',gap:12}}><label><input type="checkbox" checked={busy} onChange={event=>setBusy(event.target.checked)}/>模拟忙碌</label><button type="button" onClick={()=>setLong(!long)}>切换长内容</button></div>{long&&Array.from({length:50},(_,i)=><p key={i}>预览采集点 {i+1} · holding · u16</p>)}
 </DialogBody><DialogFooter><button type="button" disabled={busy} onClick={()=>setOpen(false)}>取消</button><button type="submit" disabled={busy}>创建</button></DialogFooter></form></WorkbenchDialog>}</main>;
}
createRoot(document.getElementById('root')!).render(new URLSearchParams(window.location.search).has('small') ? <iframe title="360 × 420 窄视口" src="/dialogs.html" style={{width:360,height:420,border:'1px solid #ccc',margin:16}}/> : <Preview/>);
