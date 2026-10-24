import {describe,it,expect} from 'vitest';
import {treeMove,beginTreeDrag,readTreeDrag,endTreeDrag,TREE_MIME,canDockTargets} from './treeDrag';
import {folderIcons,folderIcon,folderSprite,safeFolderColor} from './folderIcons';
import type {Catalog} from '../../shared/api/contracts';
const catalog={navigation:{folders:[{id:'a',scope:'models',parent:null},{id:'b',scope:'models',parent:'a'},{id:'s',scope:'simulators',parent:null}]}} as unknown as Catalog;
describe('tree drag boundary',()=>{
 it('changes parent only and rejects cycles, files and other scopes',()=>{
  expect(treeMove([{kind:'model',id:'m'}],{kind:'folder',id:'a'},'models',catalog)).toEqual({kind:'moveNavigation',nodes:[{kind:'model',id:'m'}],scope:'models',parent:'a'});
  expect(()=>treeMove([{kind:'folder',id:'a'}],{kind:'folder',id:'b'},'models',catalog)).toThrow();
  expect(()=>treeMove([{kind:'model',id:'m'}],{kind:'folder',id:'s'},'models',catalog)).toThrow();
  expect(()=>treeMove([{kind:'model',id:'m'}],{kind:'model',id:'x'},'models',catalog)).toThrow();
  expect(canDockTargets([{kind:'folder',id:'a'},{kind:'model',id:'m'}])).toBe(false);
 });
 it('accepts only a live local session, including protected dragover data',()=>{
  const data=new Map<string,string>();const transfer={types:[TREE_MIME],setData:(k:string,v:string)=>data.set(k,v),getData:(k:string)=>data.get(k)??''} as unknown as DataTransfer;
  expect(readTreeDrag(transfer)).toBeUndefined();beginTreeDrag(transfer,[{kind:'model',id:'m'}]);expect(readTreeDrag(transfer)?.[0]?.id).toBe('m');
  data.set(TREE_MIME,'foreign');expect(readTreeDrag(transfer)).toBeUndefined();data.set(TREE_MIME,'');expect(readTreeDrag(transfer)).toHaveLength(1);endTreeDrag();expect(readTreeDrag(transfer)).toBeUndefined();
 });
});
it('provides 48 stable local themes and safe legacy fallbacks',()=>{
 expect(folderIcons).toHaveLength(48);expect(new Set(folderIcons.map(i=>i.id)).size).toBe(48);
 expect(folderIcon('future-theme').id).toBe('folder');expect(safeFolderColor('url(x)')).toBe('#477fbc');
 const sprite=folderSprite([{id:'"bad',icon:'<script>',color:'url(x)'}]);expect(sprite).not.toContain('<script>');expect(sprite).not.toContain('url(x)');expect(sprite).not.toContain('id="folder-theme-"');
});

it('avoids a command for same-parent parent/descendant selections and still sends stale sources for rejection',async()=>{
 const {navigationMoveChanges}=await import('./treeDrag');
 const data={models:[{id:'m'}],endpoints:[],navigation:{folders:[{id:'a',scope:'models',parent:null},{id:'b',scope:'models',parent:'a'}],members:{},modelMembers:{m:'b'}}} as unknown as Catalog;
 const same=treeMove([{kind:'folder',id:'a'},{kind:'model',id:'m'}],undefined,'models',data);
 expect(navigationMoveChanges(same,data)).toBe(false);
 expect(navigationMoveChanges(treeMove([{kind:'model',id:'m'}],undefined,'models',data),data)).toBe(true);
 expect(navigationMoveChanges(treeMove([{kind:'model',id:'missing'}],undefined,'models',data),data)).toBe(true);
});
