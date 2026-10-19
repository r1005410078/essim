import type {CellValue} from "@zhiguang/novasheet-core";
import {SnapshotDataSource} from "./SnapshotDataSource";

/** 草稿基线与实际值分开；只有用户编辑产生待提交项。 */
export class RuntimeDataSource extends SnapshotDataSource {
 private baselines=new Map<number,string>();
 private dirty=new Set<number>();
 constructor(options:ConstructorParameters<typeof SnapshotDataSource>[0],private changed:()=>void){
  super(options);
  options.rows.forEach((row,index)=>this.baselines.set(index,String(row.draft??"")));
 }
 override updateCell(row:number,field:string,value:CellValue){
  if(field==="draft"){
   const before=this.getCell(row,field);
   if(String(value??"")===this.baselines.get(row))this.dirty.delete(row);else this.dirty.add(row);
   super.updateCell(row,field,value);
   if(before!==value)this.changed();
  }else super.updateCell(row,field,value);
 }
 pendingRows(){return [...this.dirty];}
 syncActual(row:number,text:string){
  if(this.dirty.has(row))return;
  this.baselines.set(row,text);
  if(this.getCell(row,"draft")!==text)super.updateCell(row,"draft",text);
 }
 acknowledge(row:number,sent:string){
  this.baselines.set(row,sent);
  if(String(this.getCell(row,"draft")??"")===sent)this.dirty.delete(row);
  this.changed();
 }
}
