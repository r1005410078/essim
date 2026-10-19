import guide from "../../resources/protocols/modbus/guide.md?raw";
import type {Target} from "../workspace/model";
export interface DocumentResource {id:string;title:string;folders:readonly string[];source:string;content:string}
/** 内置只读资料独立于会话配置；稳定ID用于Tab，路径只用于导航分类。 */
export const resources:readonly DocumentResource[]=[{id:"modbus-guide",title:"Modbus 详细指南",folders:["协议","Modbus"],source:"protocols/modbus/guide.md",content:guide}];
export const resourcePanelId=(id:string)=>`resource:${id}`;
export const headingId=(text:string)=>text.trim().toLowerCase().replace(/[^\p{L}\p{N}]+/gu,"-").replace(/^-|-$/g,"");
export function markdownHeadings(content:string){
 const headings:{id:string;text:string;level:number;line:number}[]=[];const counts=new Map<string,number>();let fence="";
 content.split("\n").forEach((line,index)=>{
  const marker=line.trim().match(/^(`{3,}|~{3,})/);
  if(marker){if(!fence)fence=marker[1]![0]!;else if(marker[1]![0]===fence)fence="";return;}
  if(fence)return;const match=line.match(/^(#{1,3})\s+(.+?)\s*#*$/);if(!match)return;
  const text=match[2]!.replace(/[`*_]/g,""),base=headingId(text),count=(counts.get(base)??0)+1;counts.set(base,count);
  headings.push({id:`${base}${count>1?`-${count}`:""}`,text,level:match[1]!.length,line:index+1});
 });return headings;
}
export function resourceNavigation(){
 const targets=new Map<string,Target>();const paths=resources.map(resource=>{const path=["文档资源",...resource.folders,resource.title].join("/");targets.set(path,{kind:"resource",id:resource.id});return path;});
 return {paths,targets};
}
const sections:Record<number,string>={7:"串行诊断",8:"串行诊断",11:"串行诊断",12:"串行诊断",17:"串行诊断",1:"FC01 读线圈",2:"FC02 读离散输入",3:"FC03 读保持寄存器",4:"FC04 读输入寄存器",5:"FC05 写单个线圈",6:"FC06 写单个寄存器",15:"FC15 写多个线圈",16:"FC16 写多个寄存器"};
export function resourceSectionForMessage(functionCode:number|undefined,direction:"RX"|"TX"){
 return headingId(functionCode!==undefined&&direction==="TX"&&(functionCode&0x80)!==0?"异常响应":sections[functionCode??-1]??"其他功能码与扩展");
}
