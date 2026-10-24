import type {Catalog,FolderScope,NavigationNode} from '../../shared/api/contracts';
import type {Target} from './model';
export const TREE_MIME='application/x-essim-tree';
let active:{token:string;targets:Target[]}|undefined;
export function draggableTarget(target:Target|undefined):target is Target{return !!target&&!target.unit&&['folder','model','endpoint','resource'].includes(target.kind);}
export function beginTreeDrag(transfer:DataTransfer,targets:Target[]){active={token:crypto.randomUUID(),targets:targets.map(t=>({...t}))};transfer.setData(TREE_MIME,active.token);transfer.effectAllowed='move';}
export function readTreeDrag(transfer:DataTransfer|null):Target[]|undefined{if(!active||!transfer?.types.includes(TREE_MIME))return;const token=transfer.getData(TREE_MIME);if(token&&token!==active.token)return;return active.targets.map(t=>({...t}));}
export function endTreeDrag(){active=undefined;}
export function canDockTargets(targets:Target[]|undefined):targets is Target[]{return !!targets?.length&&targets.every(target=>!target.unit&&['model','endpoint','resource'].includes(target.kind));}
export function targetScope(target:Target,catalog:Catalog):FolderScope|undefined{
 if(target.unit)return;if(target.kind==='endpoint')return 'simulators';if(target.kind==='model')return 'models';if(target.kind==='folder')return catalog.navigation?.folders.find(f=>f.id===target.id)?.scope??'simulators';
}
export function treeMove(targets:Target[],destination:Target|undefined,scope:FolderScope,catalog:Catalog){
 if(!targets.length||targets.some(t=>!draggableTarget(t)||targetScope(t,catalog)!==scope))throw Error('只能移动同一分类的文件夹、模型或模拟器');
 if(destination&&destination.kind!=='folder')throw Error('请选择文件夹或根目录');
 if(destination&&targetScope(destination,catalog)!==scope)throw Error('目标目录分类不匹配');
 const selectedFolders=new Set(targets.filter(t=>t.kind==='folder').map(t=>t.id));
 let ancestor=destination?.id;const seen=new Set<string>();
 while(ancestor){if(selectedFolders.has(ancestor))throw Error('不能移动到自身或子文件夹');if(seen.has(ancestor))throw Error('目录层级存在循环');seen.add(ancestor);ancestor=catalog.navigation?.folders.find(f=>f.id===ancestor)?.parent??undefined;}
 const nodes=targets.map(t=>({kind:t.kind,id:t.id})) as NavigationNode[];
 return {kind:'moveNavigation' as const,nodes,scope,parent:destination?.id??null};
}

/** 同父级拖放不发配置命令；失效来源仍交核心整体拒绝，不能静默当作无变化。 */
export function navigationMoveChanges(action:ReturnType<typeof treeMove>,catalog:Catalog){
 const folders=catalog.navigation?.folders??[];
 const selected=new Set(action.nodes.filter(node=>node.kind==='folder').map(node=>node.id));
 return action.nodes.some(node=>{
  const folder=node.kind==='folder'?folders.find(item=>item.id===node.id):undefined;
  const exists=node.kind==='folder'?!!folder:node.kind==='model'?catalog.models.some(item=>item.id===node.id):catalog.endpoints.some(item=>item.id===node.id);
  if(!exists)return true;
  const parent=node.kind==='folder'?folder?.parent:node.kind==='model'?catalog.navigation?.modelMembers?.[node.id]:catalog.navigation?.members[node.id];
  let ancestor=parent;const seen=new Set<string>();
  while(ancestor){if(seen.has(ancestor))return true;seen.add(ancestor);if(selected.has(ancestor))return false;ancestor=folders.find(item=>item.id===ancestor)?.parent;}
  return (parent??null)!==action.parent;
 });
}
