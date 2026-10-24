import {readTreeDrag,canDockTargets,endTreeDrag} from './treeDrag';
import type {DockviewApi,DockviewGroupPanel,IDockviewPanel} from 'dockview-react';
export type DocumentLocation={group:DockviewGroupPanel;position:'center'|'left'|'right'|'top'|'bottom'};
export function isDocument(id:string){return id==='welcome'||/^(model|endpoint|resource|tool):/.test(id);}
export function documentGroup(group:DockviewGroupPanel|undefined):group is DockviewGroupPanel{return !!group&&group.api.location.type==='grid'&&group.panels.every(p=>isDocument(p.id));}
export function dockingPosition(location:DocumentLocation){
 return {referenceGroup:location.group.id,direction:({center:'within',left:'left',right:'right',top:'above',bottom:'below'} as const)[location.position]};
}
/** 移动现有面板，保留组件、草稿与后台实例的所有权。 */
export function relocateDocument(panel:IDockviewPanel,location:DocumentLocation){
 if(!isDocument(panel.id)||!documentGroup(location.group))throw Error('只能在中央文档区分屏');
 if(panel.group.id===location.group.id&&(location.position==='center'||panel.group.panels.length===1)){panel.api.setActive();return;}
 panel.api.moveTo({group:location.group,position:location.position});panel.api.setActive();
}
/** 先完整加载，再由调用者同步修改布局；失败不产生半批分组。 */
export async function prepareDocumentDrop<T>(targets:T[],validate:(target:T)=>boolean,load:(target:T)=>Promise<void>){
 if(!targets.length||targets.some(t=>!validate(t)))throw Error('拖入的资源已失效或不是文档');
 await Promise.all(targets.map(load));
 if(targets.some(t=>!validate(t)))throw Error('加载期间资源已失效');
}
export function liveDocumentLocation(api:DockviewApi,location:DocumentLocation){return api.groups.includes(location.group)&&documentGroup(location.group);}

/** 根落区只在中央网格为空时接收文档，避免外层分割挤走固定工具组。 */
export function allowsDocumentDrop(api:DockviewApi,group:DockviewGroupPanel|undefined,kind:string,outer=false){
 if(group)return documentGroup(group)&&kind!=='edge'&&!outer;
 return !api.groups.some(item=>item.api.location.type==='grid');
}

/** 统一安装原生停靠边界，测试与实际工作台使用同一入口。 */
export function installDocumentDocking(api:DockviewApi,drop:(targets:import('./model').Target[],location?:DocumentLocation)=>void){
 const targets=(event:DragEvent|PointerEvent)=>event instanceof DragEvent?readTreeDrag(event.dataTransfer):undefined;
 const listeners=[
  api.onWillDragPanel(event=>{if(!isDocument(event.panel.id))event.nativeEvent.preventDefault();}),
  api.onWillDragGroup(event=>{if(!documentGroup(event.group))event.nativeEvent.preventDefault();}),
  api.onWillShowOverlay(event=>{if(!allowsDocumentDrop(api,event.group,event.kind,event.edge||event.edgeGroup))event.preventDefault();}),
  api.onWillDrop(event=>{if(!allowsDocumentDrop(api,event.group,event.kind))event.preventDefault();}),
  api.onUnhandledDragOver(event=>{if(canDockTargets(targets(event.nativeEvent))&&allowsDocumentDrop(api,event.group,event.target))event.accept();}),
  api.onDidDrop(event=>{
   const batch=targets(event.nativeEvent);
   if(!canDockTargets(batch)||!allowsDocumentDrop(api,event.group,event.group?'content':'edge'))return;
   endTreeDrag();drop(batch,event.group?{group:event.group,position:event.position}:undefined);
  }),
 ];
 return {dispose:()=>listeners.forEach(listener=>listener.dispose())};
}

/** 显式定位中央文档，避免最后一个文档关闭后使用边栏作为默认组。 */
export function defaultDocumentPosition(api:DockviewApi){
 const group=api.groups.find(documentGroup)??api.addGroup();
 return {referenceGroup:group.id,direction:'within' as const};
}
