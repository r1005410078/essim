import type {DataSource,DataSourceListener,CellValue,Row} from '@zhiguang/novasheet-core';

/** 仅为运行网格补充无ID空行，真实数据源和运行草稿保持原样。 */
export class RuntimeGridSource implements DataSource {
 private minimumRows=0;
 private listeners=new Set<DataSourceListener>();
 constructor(private readonly source:DataSource){}
 getSchema(){return this.source.getSchema();}
 getRowCount(){return Math.max(this.source.getRowCount(),this.minimumRows);}
 getCell(row:number,field:string){return row<this.source.getRowCount()?this.source.getCell(row,field):undefined;}
 getRows(start:number,end:number):Row[]|Promise<Row[]>{
  const from=Math.max(0,start),to=Math.min(end,this.getRowCount()-1);
  if(to<from)return [];
  const actualEnd=Math.min(to,this.source.getRowCount()-1);
  const actual=actualEnd>=from?this.source.getRows(from,actualEnd):[];
  const pad=(rows:Row[])=>[...rows,...Array.from({length:to-Math.max(actualEnd,from-1)},()=>({}))];
  return actual instanceof Promise?actual.then(pad):pad(actual);
 }
 updateCell(row:number,field:string,value:CellValue){
  if(row<0||row>=this.source.getRowCount())return;
  const editable=this.source as DataSource&{updateCell?:(row:number,field:string,value:CellValue)=>void};
  editable.updateCell?.(row,field,value);
 }
 /** 本包装不重排行，视图与底层行号相同；仍经真实源的草稿写入路径。 */
 updateCellByUnderlyingRow(row:number,field:string,value:CellValue){this.updateCell(row,field,value);}
 subscribe(listener:DataSourceListener){
  this.listeners.add(listener);
  const unsubscribe=this.source.subscribe(event=>listener(event.type==='rowCountChanged'?{...event,newCount:this.getRowCount()}:event));
  return()=>{this.listeners.delete(listener);unsubscribe();};
 }
 /** 根据可视高度补足一屏，包含最后一行的部分可见区域。 */
 fitHeight(height:number,rowHeight:number,headerHeight:number){
  if(height<=0)return; // 隐藏分类不因display:none重置可见网格。
  const count=Math.max(0,Math.ceil((height-headerHeight)/rowHeight));
  const previous=this.getRowCount();this.minimumRows=count;
  if(this.getRowCount()!==previous){
   for(const listener of [...this.listeners])listener({type:'rowCountChanged',newCount:this.getRowCount()});
   // 与NovaSheet setRows契约一致，让隐藏行等视图层同步重算行映射。
   for(const listener of [...this.listeners])listener({type:'reset'});
  }
 }
}
