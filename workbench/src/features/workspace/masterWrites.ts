import {slaveRaw} from "./slaveValues";
import type {PointDefinition,RawValue} from "../../shared/api/contracts";
import type {MasterView} from "../../shared/api/contracts";
import type {SheetViews,PendingRuntimeEdit} from "./sheets";
/** 仅使用同一操作的最终回验结果确认已发送草稿；期间的新编辑继续保留。 */
export function acknowledgeMasterWrites(sheets:SheetViews,operation:string,edits:PendingRuntimeEdit[],view:MasterView){
 if(view.busy||view.operation!==operation)return;
 sheets.acknowledge(edits.filter(edit=>{const point=view.points[edit.point.id];return point?.writeOperation===operation&&point.writeResult==="协议已确认，回验一致";}));
}

/** f32草稿按目标IEEE754单精度规范化；整数继续保持精确文本。 */
export function masterRaw(point:PointDefinition,text:string):RawValue{
 const raw=slaveRaw(point,text);
 if(point.valueType==="f32"&&raw.kind==="float"){
  const value=Math.fround(Number(raw.value));
  if(!Number.isFinite(value))throw {code:"VALUE_OVERFLOW",message:`${point.name}：数值超出f32范围`,pointId:point.id};
  return {kind:"float",value:String(value)};
 }
 return raw;
}
