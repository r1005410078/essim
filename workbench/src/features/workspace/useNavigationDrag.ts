import {attachTreeEvents} from './treeMount';
import {useEffect,useRef} from 'react';
import type {FileTreeProps} from '@pierre/trees/react';
import type {Catalog,FolderScope} from '../../shared/api/contracts';
import {type WorkspaceStore,type Target,type projectNavigation,errorMessage} from './model';
import {beginTreeDrag,readTreeDrag,endTreeDrag,draggableTarget,treeMove,navigationMoveChanges} from './treeDrag';
/** 仅适配当前树库事件边界；业务移动在核心完成后由权威目录重绘。 */
export function useNavigationDrag(model:FileTreeProps['model'],input:ReturnType<typeof projectNavigation>,catalog:Catalog|undefined,store:WorkspaceStore,section:'simulators'|'resources',open:(target:Target)=>void){
 const current=useRef({input,catalog,store,section,open});current.current={input,catalog,store,section,open};
 useEffect(()=>{
  return attachTreeEvents(()=>model.getFileTreeContainer(),(host,root)=>{
   let highlighted:HTMLElement|undefined;let hovered:string|undefined;let hoverTimer:ReturnType<typeof setTimeout>|undefined;
   const rowOf=(event:Event)=>event.composedPath().find(item=>item instanceof HTMLElement&&item.dataset.itemPath) as HTMLElement|undefined;
   const targetOf=(row:HTMLElement|undefined)=>row?current.current.input.targets.get(row.dataset.itemPath!):undefined;
   const clear=()=>{highlighted?.removeAttribute('data-navigation-drop');highlighted=undefined;hovered=undefined;clearTimeout(hoverTimer);};
   const start=(event:DragEvent)=>{const target=targetOf(rowOf(event));if(!draggableTarget(target)||!event.dataTransfer||store.busy){event.preventDefault();return;}
    const selected=model.getSelectedPaths();const path=rowOf(event)?.dataset.itemPath!;
    const order=new Map(model.getVisibleRows(0,model.getVisibleCount()).map((row,index)=>[row.path,index]));
    const paths=(selected.includes(path)?[...selected]:[path]).sort((a,b)=>(order.get(a)??Infinity)-(order.get(b)??Infinity));const selectedTargets=paths.map(p=>current.current.input.targets.get(p));
    if(!selectedTargets.every(draggableTarget)){event.preventDefault();return;}
    beginTreeDrag(event.dataTransfer,selectedTargets);
   };
   const plan=(event:DragEvent)=>{
    const targets=readTreeDrag(event.dataTransfer);if(!targets||!current.current.catalog)return;
    const row=rowOf(event);const destination=targetOf(row);
    // 固定“采集模型”根分组可作为模型根目标，其他固定分组不可接收移动。
    if(row&&!destination&&row.dataset.itemPath?.replace(/\/$/,'')!=='采集模型')return;
    const scope:FolderScope=current.current.section==='simulators'?'simulators':'models';
    return {action:treeMove(targets,destination,scope,current.current.catalog),row};
   };
   const over=(event:DragEvent)=>{
    if(!readTreeDrag(event.dataTransfer))return;
    event.stopImmediatePropagation();
    try{const next=plan(event);if(!next||store.busy){clear();return;}event.preventDefault();if(event.dataTransfer)event.dataTransfer.dropEffect='move';
     const scroll=root.querySelector<HTMLElement>('[data-file-tree-virtualized-scroll=true]');if(scroll){const rect=scroll.getBoundingClientRect();if(event.clientY<rect.top+28)scroll.scrollTop-=12;else if(event.clientY>rect.bottom-28)scroll.scrollTop+=12;}
     if(highlighted!==next.row){highlighted?.removeAttribute('data-navigation-drop');highlighted=next.row;highlighted?.setAttribute('data-navigation-drop','true');}
     const path=next.row?.dataset.itemPath;if(path&&hovered!==path){clearTimeout(hoverTimer);hovered=path;hoverTimer=setTimeout(()=>{const item=model.getItem(path);if(item&&'expand' in item)item.expand();},600);}
    }catch{clear();if(event.dataTransfer)event.dataTransfer.dropEffect='none';}
   };
   const drop=(event:DragEvent)=>{
    if(!readTreeDrag(event.dataTransfer))return;
    event.preventDefault();event.stopImmediatePropagation();
    try{const next=plan(event);if(!next||store.busy||!navigationMoveChanges(next.action,current.current.catalog!))return;void store.execute(next.action).catch(error=>{store.error=errorMessage(error);store.changed();});}
    catch(error){store.error=errorMessage(error);store.changed();}finally{clear();endTreeDrag();}
   };
   const click=(event:MouseEvent)=>{if(event.ctrlKey||event.metaKey||event.shiftKey||event.button!==0)return;const row=rowOf(event);const target=targetOf(row);if(target&&!(event.composedPath().some(item=>item instanceof HTMLElement&&item.dataset.itemSection==='icon'&&target.kind==='folder')))current.current.open(target);};
   const key=(event:KeyboardEvent)=>{if(event.key==='Escape'){clear();endTreeDrag();}if(event.key==='Enter'&&!event.metaKey&&!event.ctrlKey){const target=targetOf(rowOf(event));if(target)current.current.open(target);}};
   const leave=(event:DragEvent)=>{const rect=host!.getBoundingClientRect();if(event.clientX<=rect.left||event.clientX>=rect.right||event.clientY<=rect.top||event.clientY>=rect.bottom)clear();};
   const end=()=>{clear();endTreeDrag();};
   root.addEventListener('dragstart',start as EventListener,true);root.addEventListener('dragover',over as EventListener,true);root.addEventListener('drop',drop as EventListener,true);root.addEventListener('dragleave',leave as EventListener,true);root.addEventListener('dragend',end,true);root.addEventListener('click',click as EventListener,true);root.addEventListener('keydown',key as EventListener,true);
   return()=>{clear();root.removeEventListener('dragstart',start as EventListener,true);root.removeEventListener('dragover',over as EventListener,true);root.removeEventListener('drop',drop as EventListener,true);root.removeEventListener('dragleave',leave as EventListener,true);root.removeEventListener('dragend',end,true);root.removeEventListener('click',click as EventListener,true);root.removeEventListener('keydown',key as EventListener,true);};
  });
 },[model,store]);
}
