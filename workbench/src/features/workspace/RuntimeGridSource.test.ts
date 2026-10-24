import {expect,it} from 'vitest';
import {RuntimeDataSource} from './RuntimeDataSource';
import {RuntimeGridSource} from './RuntimeGridSource';

function setup(rows:Record<string,string>[]=[]){
 const actual=new RuntimeDataSource({schema:{fields:[{id:'draft',type:'text',name:'草稿',width:100}]},rows},()=>{});
 return {actual,view:new RuntimeGridSource(actual)};
}
it('空表和不足一屏的运行数据补足可视行，但不创建真实点',()=>{
 const {actual,view}=setup([{id:'p1',draft:'12'}]);
 view.fitHeight(1026,25,25);
 expect(view.getRowCount()).toBe(41);
 expect(view.getRows(0,2)).toEqual([{id:'p1',draft:'12'},{},{}]);
 expect(view.getRows(39,100)).toEqual([{},{}]);
 expect(actual.getRowCount()).toBe(1);
 const empty=setup().view;empty.fitHeight(500,25,25);
 expect(empty.getRowCount()).toBe(19);
 expect(empty.getRows(0,1)).toEqual([{},{}]);
});
it('填充行不可写且无ID，真实行仍产生运行草稿',()=>{
 const {actual,view}=setup([{id:'p1',draft:'12'}]);view.fitHeight(500,25,25);
 view.updateCell(4,'draft','99');
 expect(actual.pendingRows()).toEqual([]);
 expect(view.getCell(4,'id')).toBeUndefined();
 view.updateCellByUnderlyingRow(0,'draft','15');expect(actual.pendingRows()).toEqual([0]);
 view.updateCellByUnderlyingRow(4,'draft','99');expect(actual.pendingRows()).toEqual([0]);
});
it('缩放只调整填充行，真实数据多于视口时完整保留，隐藏不重置',()=>{
 const {view}=setup(Array.from({length:30},(_,i)=>({id:String(i)})));
 const events:unknown[]=[];const stop=view.subscribe(e=>events.push(e));
 view.fitHeight(1025,25,25);expect(view.getRowCount()).toBe(40);
 view.fitHeight(0,25,25);expect(view.getRowCount()).toBe(40);
 view.fitHeight(200,25,25);expect(view.getRowCount()).toBe(30);
 expect(events).toEqual([{type:'rowCountChanged',newCount:40},{type:'reset'},{type:'rowCountChanged',newCount:30},{type:'reset'}]);stop();
});
