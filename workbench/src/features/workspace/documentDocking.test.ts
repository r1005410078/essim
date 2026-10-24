import {describe,it,expect,vi} from 'vitest';
import {documentGroup,dockingPosition,relocateDocument,prepareDocumentDrop} from './documentDocking';
import type {DockviewGroupPanel,IDockviewPanel} from 'dockview-react';
const group=(id:string,ids:string[],type='grid')=>({id,panels:ids.map(id=>({id})),api:{location:{type}}}) as DockviewGroupPanel;
describe('document docking',()=>{
 it('allows central groups only and translates directions',()=>{
  const g=group('a',['model:m']);expect(documentGroup(g)).toBe(true);expect(documentGroup(group('tools',['inspector-point'],'edge'))).toBe(false);
  expect(dockingPosition({group:g,position:'top'})).toEqual({referenceGroup:'a',direction:'above'});expect(dockingPosition({group:g,position:'bottom'}).direction).toBe('below');
 });
 it('moves the same instance and avoids splitting its sole tab from itself',()=>{
  const source=group('a',['model:m']);const moveTo=vi.fn(),setActive=vi.fn();const panel={id:'model:m',group:source,api:{moveTo,setActive}} as unknown as IDockviewPanel;
  relocateDocument(panel,{group:source,position:'left'});expect(moveTo).not.toHaveBeenCalled();
  const target=group('b',['endpoint:e']);relocateDocument(panel,{group:target,position:'right'});expect(moveTo).toHaveBeenCalledWith({group:target,position:'right'});
  expect(()=>relocateDocument(panel,{group:group('bad',['project'],'edge'),position:'center'})).toThrow();
 });
 it('preflights the whole batch including expiry during load',async()=>{
  const load=vi.fn(async(id:string)=>{if(id==='b')throw Error('load failed');});await expect(prepareDocumentDrop(['a','b'],()=>true,load)).rejects.toThrow('load failed');
  let valid=true;await expect(prepareDocumentDrop(['a'],()=>valid,async()=>{valid=false;})).rejects.toThrow();load.mockClear();await expect(prepareDocumentDrop(['bad'],()=>false,load)).rejects.toThrow();expect(load).not.toHaveBeenCalled();
 });
});

it('accepts an empty central workspace but never an outer edge beside existing documents',async()=>{
 const {allowsDocumentDrop}=await import('./documentDocking');
 const empty={groups:[group('tools',['project'],'edge')]} as unknown as import('dockview-react').DockviewApi;
 expect(allowsDocumentDrop(empty,undefined,'edge',true)).toBe(true);
 const central=group('a',['welcome']);const occupied={groups:[central,...empty.groups]} as unknown as import('dockview-react').DockviewApi;
 expect(allowsDocumentDrop(occupied,undefined,'edge',true)).toBe(false);
 expect(allowsDocumentDrop(occupied,central,'content',false)).toBe(true);
 expect(allowsDocumentDrop(empty,empty.groups[0],'content',false)).toBe(false);
});
it('installs the actual preview/drop guards and captures the local batch before dragend',async()=>{
 const {installDocumentDocking}=await import('./documentDocking');
 const {beginTreeDrag,endTreeDrag,TREE_MIME}=await import('./treeDrag');
 const handlers=new Map<string,(event:any)=>void>();const dispose=vi.fn();
 const api={groups:[group('documents',['welcome'])],...Object.fromEntries(['onWillDragPanel','onWillDragGroup','onWillShowOverlay','onWillDrop','onUnhandledDragOver','onDidDrop'].map(name=>[name,(handler:(e:any)=>void)=>{handlers.set(name,handler);return {dispose};}]))} as unknown as import('dockview-react').DockviewApi;
 class NativeDrag{constructor(public dataTransfer:DataTransfer){}}
 vi.stubGlobal('DragEvent',NativeDrag);
 const data=new Map<string,string>();const transfer={types:[TREE_MIME],setData:(k:string,v:string)=>data.set(k,v),getData:(k:string)=>data.get(k)??''} as unknown as DataTransfer;
 const dropped=vi.fn();const bound=installDocumentDocking(api,dropped);const accept=vi.fn(),preventDefault=vi.fn();
 try{
  beginTreeDrag(transfer,[{kind:'model',id:'a'},{kind:'model',id:'b'}]);
  const event={nativeEvent:new NativeDrag(transfer),group:api.groups[0],target:'content',position:'left',accept,preventDefault};
  handlers.get('onUnhandledDragOver')!(event);expect(accept).toHaveBeenCalledOnce();
  handlers.get('onDidDrop')!(event);endTreeDrag();expect(dropped).toHaveBeenCalledWith([{kind:'model',id:'a'},{kind:'model',id:'b'}],{group:api.groups[0],position:'left'});
  accept.mockClear();beginTreeDrag(transfer,[{kind:'folder',id:'folder'},{kind:'model',id:'a'}]);handlers.get('onUnhandledDragOver')!(event);expect(accept).not.toHaveBeenCalled();
  handlers.get('onWillDrop')!({...event,group:group('inspector',['inspector-point'],'edge'),kind:'content'});expect(preventDefault).toHaveBeenCalledOnce();
  handlers.get('onWillDragPanel')!({panel:{id:'project'},nativeEvent:{preventDefault}});expect(preventDefault).toHaveBeenCalledTimes(2);
 }finally{endTreeDrag();bound.dispose();vi.unstubAllGlobals();}
 expect(dispose).toHaveBeenCalledTimes(6);
});

it('creates a central group when only fixed edge groups remain',async()=>{
 const {defaultDocumentPosition}=await import('./documentDocking');
 const central=group('new-grid',[]);const addGroup=vi.fn(()=>central);
 const api={groups:[group('tools',['inspector-point'],'edge')],addGroup} as unknown as import('dockview-react').DockviewApi;
 expect(defaultDocumentPosition(api)).toEqual({referenceGroup:'new-grid',direction:'within'});expect(addGroup).toHaveBeenCalledOnce();
 addGroup.mockClear();api.groups.push(central);expect(defaultDocumentPosition(api).referenceGroup).toBe('new-grid');expect(addGroup).not.toHaveBeenCalled();
});
