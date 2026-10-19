import type {PointDefinition,RawValue} from "../../shared/api/contracts";
/** 运行表草稿是原始值文本；保留64位整数文本，最终类型及范围由核心校验。 */
export function slaveRaw(point:PointDefinition,text:unknown):RawValue {
 const value=String(text ?? "").trim();
 const fail=()=>{throw {code:"INVALID_RAW_VALUE",message:`${point.name}：原始值格式非法`,pointId:point.id,field:"value"};};
 if(point.valueType==="bool") {
  if(value==="true"||value==="1")return {kind:"bool",value:true};
  if(value==="false"||value==="0")return {kind:"bool",value:false};
  return fail();
 }
 if(point.valueType.startsWith("f")) {if(!value||!Number.isFinite(Number(value)))return fail();return {kind:"float",value};}
 if(!/^[+-]?\d+$/.test(value))return fail();
 return {kind:point.valueType.startsWith("i")?"signed":"unsigned",value};
}
