import {widths} from "./pointDraft";
/** 从公开视口区域解析稳定身份；无选区依赖，多个冻结区域去重。 */
export function runtimeVisiblePoints(regions:readonly {rowRange:readonly [number,number]}[],pointAt:(row:number)=>string|undefined):string[]{
 const ids=new Set<string>();
 for(const {rowRange:[first,last]} of regions){
  // Excel空白区不应使扫描无界。
  const end=Math.min(last,Math.max(0,first)+1023);
  for(let row=Math.max(0,first);row<=end&&ids.size<1024;row++){
   const id=pointAt(row);if(id)ids.add(id);
  }
 }
 return [...ids];
}
/** 范围按零基地址闭区间解析；不扩大到其他数据区或半个多寄存器点。 */
export function resetRangePoints(points:readonly {id:string;area:string;address:number;valueType:string}[],area:string,start:number,end:number):string[]{
 if(!Number.isInteger(start)||!Number.isInteger(end)||start<0||end<start||end>65535)throw Error("请输入0～65535内的递增地址范围");
 const selected=points.filter(point=>point.area===area&&point.address<=end&&point.address+(widths[point.valueType]??1)-1>=start);
 let expected=start;
 for(const point of [...selected].sort((a,b)=>a.address-b.address)){
  const last=point.address+(widths[point.valueType]??1)-1;
  if(point.address!==expected||last>end)throw Error("范围包含未定义地址或截断了多寄存器点，请选择完整点范围");
  expected=last+1;
 }
 if(!selected.length||expected!==end+1)throw Error("范围包含未定义地址");
 return selected.map(point=>point.id);
}
