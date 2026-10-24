import {pinyin} from 'pinyin-pro';
import type {Catalog} from '../../shared/api/contracts';
import {projectNavigation, type Target} from '../workspace/model';
export interface ResourceSearchResult {key:string;title:string;path:string;target:Target}
const normalize=(text:string)=>text.normalize('NFKC').toLocaleLowerCase().replace(/\s+/g,'');
function keys(text:string){return [normalize(text),normalize(pinyin(text,{toneType:'none'})),normalize(pinyin(text,{pattern:'first',toneType:'none'}))];}
/** 只索引目录元数据，不读取点表正文或运行值；标签未变化时复用拼音结果。 */
export class ResourceSearchIndex {
 private entries:(ResourceSearchResult & {nameKeys:string[];pathKeys:string[]})[]=[];
 private signature='';
 update(catalog:Catalog|undefined,documents:readonly import("../../shared/api/project").ProjectDocumentInfo[]=[]){
  const unique=new Map<string,ResourceSearchResult>();
  for(const section of ['resources','simulators'] as const){
   for(const [path,target] of projectNavigation(catalog,section,documents).targets){
    if(target.kind==='folder')continue;
    const key=JSON.stringify([target.kind,target.id,target.unit??'']);
    if(!unique.has(key))unique.set(key,{key,title:path.replace(/\/$/,'').split('/').at(-1)??path,path: path.replace(/\/$/,''),target});
   }
  }
  const entries=[...unique.values()];const signature=JSON.stringify(entries);
  if(signature===this.signature)return;this.signature=signature;
  this.entries=entries.map(entry=>({...entry,nameKeys:keys(entry.title),pathKeys:keys(entry.path)}));
 }
 search(query:string,limit=100):ResourceSearchResult[]{
  const tokens=query.trim().split(/\s+/).map(normalize).filter(Boolean);
  return this.entries.map((entry,index)=>{
   let rank=0;
   for(const token of tokens){
    const scores=entry.nameKeys.map((key,i)=>key===token?i:key.startsWith(token)?3+i:key.includes(token)?6+i:Infinity);
    const score=Math.min(...scores,...entry.pathKeys.map(key=>key.includes(token)?12:Infinity));rank+=score;
   }
   return {entry,index,rank};
  }).filter(item=>Number.isFinite(item.rank)).sort((a,b)=>a.rank-b.rank||a.index-b.index).slice(0,Math.max(0,limit)).map(({entry})=>({key:entry.key,title:entry.title,path:entry.path,target:entry.target}));
 }
}
export {parseQuickQuery} from './quickQuery';
