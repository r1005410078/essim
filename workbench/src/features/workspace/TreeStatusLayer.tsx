import {useEffect,useRef,useState} from 'react';
import {createPortal} from 'react-dom';
import type {FileTreeProps} from '@pierre/trees/react';
import type {Catalog} from '../../shared/api/contracts';
import {WorkspaceStore,isEndpointApplied,type projectNavigation} from './model';
import {queryTreeMetadata} from './treeMetadata';
import {treeStatuses,type TreeRuntime,type TreeStatus} from './treeStatus';
import {TreeStatusIcons} from './TreeStatusIcons';
interface Mount {id:number;path:string;row:HTMLElement;host:HTMLSpanElement;}
/** 第三方树无第二个React装饰槽，适配层只追加自有容器，不修改树模型。 */
export function TreeStatusLayer({model,input,store,catalog,active}:{model:FileTreeProps['model'];input:ReturnType<typeof projectNavigation>;store:WorkspaceStore;catalog?:Catalog;active:boolean}){
 const [mounts,setMounts]=useState<Mount[]>([]);
 const [runtime,setRuntime]=useState<Map<string,TreeRuntime>>(new Map());
 const latest=useRef({input,store,catalog,active});latest.current={input,store,catalog,active};
 const mounted=useRef<Mount[]>([]);
 const serial=useRef(0);
 useEffect(()=>{
  let stopped=false,frame=0;let observer:MutationObserver|undefined;let root:ShadowRoot|HTMLElement|undefined;
  const sync=()=>{
   if(stopped)return;
   const container=model.getFileTreeContainer();const nextRoot=container?.shadowRoot??container;
   if(!nextRoot){frame=requestAnimationFrame(sync);return;}
   if(root!==nextRoot){observer?.disconnect();root=nextRoot;observer=new MutationObserver(()=>{cancelAnimationFrame(frame);frame=requestAnimationFrame(sync);});observer.observe(root,{subtree:true,childList:true,attributes:true,attributeFilter:['data-item-path']});}
   const rows=[...root.querySelectorAll<HTMLElement>('[data-item-path]:not([data-item-parked])')];
   const next:Mount[]=[];
   for(const row of rows){
    const path=row.dataset.itemPath;if(!path||!latest.current.input.targets.has(path))continue;
    let mount=mounted.current.find(item=>item.row===row&&item.path===path&&item.host.parentElement===row);
    if(!mount){const host=document.createElement('span');host.className='navigation-status';row.append(host);mount={id:serial.current++,path,row,host};}
    next.push(mount);
   }
   for(const old of mounted.current)if(!next.includes(old))old.host.remove();
   if(next.length!==mounted.current.length||next.some((item,index)=>item!==mounted.current[index])){mounted.current=next;setMounts(next);}
  };
  frame=requestAnimationFrame(sync);
  return()=>{stopped=true;cancelAnimationFrame(frame);observer?.disconnect();for(const mount of mounted.current)mount.host.remove();mounted.current=[];};
 },[model,input]);
 useEffect(()=>{
  if(!active)return;
  let cancelled=false;let timer:ReturnType<typeof setTimeout>;
  const poll=async()=>{
   const context=latest.current;const sequence=context.store.view?.sequence;
   if(sequence!==undefined&&document.visibilityState!=='hidden'&&mounted.current.some(mount=>mount.row.getClientRects().length)){
    const ids=new Set(mounted.current.filter(m=>m.row.getClientRects().length).flatMap(m=>{const target=context.input.targets.get(m.path);return target?.kind==='endpoint'?[target.id]:[];}));
    const endpoints=(context.catalog?.endpoints??[]).filter(endpoint=>ids.has(endpoint.id));
    const valid=()=>!cancelled&&latest.current.active&&document.visibilityState!=='hidden'&&mounted.current.some(mount=>mount.row.getClientRects().length)&&context.store.view?.sequence===sequence;
    await queryTreeMetadata(context.store.client,endpoints,sequence,(id,value)=>{
     if(!latest.current.catalog?.endpoints.some(endpoint=>endpoint.id===id))return;
     setRuntime(previous=>new Map([...previous,[id,value]]));
    },valid);
   }
   if(!cancelled)timer=setTimeout(()=>void poll(),1000);
  };
  void poll();return()=>{cancelled=true;clearTimeout(timer);};
 },[active,store]);
 return <>{mounts.map(({id,path,host,row})=>{
  const target=input.targets.get(path);let statuses:TreeStatus[]=[];
  if(target?.kind==='endpoint'){
   const endpoint=store.endpoint(target.id)??catalog?.endpoints.find(e=>e.id===target.id);
   if(endpoint){
    const unit=target.unit?endpoint.units.find(u=>u.id===target.unit):undefined;
    const saved=catalog?.endpoints.find(e=>e.id===target.id);
    const observed=target.unit?store.observedUnitQuality.get(target.unit):undefined;
    statuses=treeStatuses({endpoint,unit,pollingEnabled:!!catalog?.applied.find(e=>e.id===endpoint.id)?.units.some(u=>u.id===unit?.id&&u.enabled),runtime:runtime.get(endpoint.id),sequence:store.view?.sequence,dirty:!target.unit&&store.endpointDirty(endpoint.id),pending:!target.unit&&!!saved&&!isEndpointApplied(catalog,saved,store.view?.applicationMatches),quality:observed?.sequence===store.view?.sequence?observed?.quality:undefined});
   }
  }else if(target?.kind==='model'&&store.modelDrafts.get(target.id)?.dirty)statuses=[{kind:'dirty',description:'存在未保存修改'}];
  return createPortal(<RowStatus row={row} statuses={statuses}/>,host,String(id));
 })}</>;
}

function RowStatus({row,statuses}:{row:HTMLElement;statuses:TreeStatus[]}){
 const description=statuses.map(status=>status.description).join('；');
 useEffect(()=>{
  if(description)row.setAttribute('aria-description',description);else row.removeAttribute('aria-description');
  return()=>{if(row.getAttribute('aria-description')===description)row.removeAttribute('aria-description');};
 },[row,description]);
 return <TreeStatusIcons statuses={statuses}/>;
}
