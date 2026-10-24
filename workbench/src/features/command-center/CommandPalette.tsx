import {useEffect,useId,useRef,useState} from 'react';
import {WorkbenchDialog,DialogBody,DialogFooter} from '@essim/ui/components/workbench-dialog';
import type {ResourceSearchResult} from './resourceSearch';
import {parseQuickQuery} from './quickQuery';
export interface PaletteItem {id:string;title:string;detail?:string;enabled:boolean;run:()=>Promise<unknown>|unknown}
export interface CommandPaletteProps {initial:string;searchResources:(query:string)=>Promise<ResourceSearchResult[]>;searchCommands:(query:string)=>{id:string;title:string;category?:string;enabled:boolean}[];executeCommand:(id:string)=>Promise<unknown>;openResource:(result:ResourceSearchResult)=>void;close:()=>void}
/** 复用公共弹框；序号屏蔽迟到查询，IME输入不触发提交。 */
export function CommandPalette({initial,searchResources,searchCommands,executeCommand,openResource,close}:CommandPaletteProps){
 const [query,setQuery]=useState(initial),[items,setItems]=useState<PaletteItem[]>([]),[selected,setSelected]=useState(0),[loading,setLoading]=useState(false),[error,setError]=useState(''),[busy,setBusy]=useState(false);
 const request=useRef(0),alive=useRef(true),listId=useId();
 const providers=useRef({searchResources,searchCommands,executeCommand,openResource});providers.current={searchResources,searchCommands,executeCommand,openResource};
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;request.current++;};},[]);
 useEffect(()=>{const sequence=++request.current;setItems([]);setSelected(0);setError('');setLoading(true);const timer=setTimeout(()=>{const parsed=parseQuickQuery(query);const p=providers.current;void Promise.resolve().then(async()=>parsed.mode==='commands'?p.searchCommands(parsed.text).map(command=>({id:command.id,title:command.title,detail:command.category,enabled:command.enabled,run:()=>providers.current.executeCommand(command.id)})):(await p.searchResources(parsed.text)).map(resource=>({id:resource.key,title:resource.title,detail:resource.path,enabled:true,run:()=>providers.current.openResource(resource)}))).then(result=>{if(sequence===request.current){setItems(result);setLoading(false);}}).catch(e=>{if(sequence===request.current){setError(String(e.message??e));setLoading(false);}});},100);return()=>{clearTimeout(timer);request.current++;};},[query]);
 const execute=async(item:PaletteItem|undefined)=>{if(!item?.enabled||busy||loading)return;setBusy(true);try{await item.run();if(alive.current)close();}catch(e){if(alive.current)setError(String((e as Error).message??e));}finally{if(alive.current)setBusy(false);}};
 useEffect(()=>{document.getElementById(`${listId}-${selected}`)?.scrollIntoView({block:'nearest'});},[selected,listId]);
 return <WorkbenchDialog variant="command" aria-label="全局资源与命令检索" title="全局资源与命令检索" initialWidth={640} busy={busy} onCancel={close}>
 <form onSubmit={event=>{event.preventDefault();void execute(items[selected]);}}>
 <DialogBody><input autoFocus aria-label="资源或命令" className="command-input" value={query} disabled={busy} placeholder="搜索资源；输入 > 执行命令" role="combobox" aria-expanded="true" aria-controls={listId} aria-activedescendant={items[selected]?`${listId}-${selected}`:undefined} onChange={e=>{request.current++;setItems([]);setLoading(true);setSelected(0);setQuery(e.target.value);}} onKeyDown={event=>{if(event.nativeEvent.isComposing){if(event.key==='Enter')event.preventDefault();return;}if(event.key==='ArrowDown'||event.key==='ArrowUp'){event.preventDefault();setSelected(i=>Math.max(0,Math.min(items.length-1,i+(event.key==='ArrowDown'?1:-1))));}if(event.key==='Enter'&&(event.nativeEvent.isComposing||event.keyCode===229))event.preventDefault();}}/>
 <div id={listId} role="listbox" aria-label="检索结果" className="command-results" aria-busy={loading}>{items.map((item,index)=><div key={item.id} id={`${listId}-${index}`} role="option" aria-selected={index===selected} aria-disabled={!item.enabled} className={`command-result ${index===selected?'selected':''}`} onMouseDown={event=>event.preventDefault()} onClick={()=>{setSelected(index);void execute(item);}}><span>{item.title}</span><small>{item.detail}{!item.enabled?' · 当前不可用':''}</small></div>)}</div>
 {loading?<p role="status">正在检索…</p>:!items.length&&!error?<p role="status">没有匹配结果</p>:null}{error&&<p role="alert" className="error">{error}</p>}</DialogBody>
 <DialogFooter><span className="command-hint">↑↓ 选择 · Enter 打开／执行 · &gt; 命令模式</span><button type="button" disabled={busy} onClick={close}>取消</button><button type="submit" disabled={busy||loading||!items[selected]?.enabled}>打开／执行</button></DialogFooter>
 </form></WorkbenchDialog>;
}
