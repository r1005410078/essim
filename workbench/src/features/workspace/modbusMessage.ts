/** 仅用于展示真实TCP ADU；协议执行仍由Rust/libmodbus负责。 */
export interface MessageField {section:"MBAP"|"PDU"|"RTU";hex:string;description:string;tone:"identity"|"neutral"|"unit"|"function"|"address"|"value";offset:number}
export interface DecodedMessage {title:string;subtitle:string;fields:MessageField[];issues:string[];values:{label:string;value:string}[];valueCount:number;note:string}
const functions:Record<number,string>={1:"读线圈",2:"读离散输入",3:"读保持寄存器",4:"读输入寄存器",5:"写单个线圈",6:"写单个寄存器",15:"写多个线圈",16:"写多个寄存器"};
const exceptions:Record<number,string>={1:"非法功能",2:"非法数据地址",3:"非法数据值",4:"服务器设备故障",5:"确认，处理尚未完成",6:"服务器设备忙",8:"存储奇偶校验错误",10:"网关路径不可用",11:"网关目标设备未响应"};
export const messageHex=(bytes:readonly number[])=>bytes.map(byte=>byte.toString(16).padStart(2,"0").toUpperCase()).join(" ");
function bytesFromHex(hex:string):number[]|undefined {
 const parts=hex.trim().split(/\s+/);return parts.length<=260&&parts.every(part=>/^[\da-f]{2}$/i.test(part))?parts.map(part=>Number.parseInt(part,16)):undefined;
}
const u16=(bytes:number[],offset:number)=>(bytes[offset]!<<8)|bytes[offset+1]!;
/** 仅在头部、功能码及请求结构均有效且一致时借用请求地址。 */
function readContext(bytes:number[],requestHex?:string):{address:number;quantity:number}|undefined {
 if(!requestHex)return;
 const request=bytesFromHex(requestHex);
 if(!request||request.length!==12||u16(request,2)!==0||u16(request,4)!==6||request[6]!==bytes[6]||u16(request,0)!==u16(bytes,0)||request[7]!==bytes[7])return;
 const quantity=u16(request,10),address=u16(request,8),limit=bytes[7]!<3?2000:125;
 if(quantity<1||quantity>limit||address+quantity>65536)return;
 return {address,quantity};
}
/** 按官方应用协议V1.1b3第6/7节及TCP指南解析；地址一律零基，寄存器只展示原始u16。 */
export function decodeModbusMessage(hex:string,direction:"RX"|"TX",requestHex?:string):DecodedMessage {
 const result:DecodedMessage={title:"Modbus TCP",subtitle:direction==="RX"?"请求":"响应",fields:[],issues:[],values:[],valueCount:0,note:""};
 const bytes=bytesFromHex(hex);
 if(!bytes){result.issues.push("字节格式无效或超过TCP ADU上限260字节");return result;}
 const field=(section:MessageField["section"],offset:number,length:number,description:string,tone:MessageField["tone"]="neutral")=>{
  if(offset+length<=bytes.length)result.fields.push({section,offset,hex:messageHex(bytes.slice(offset,offset+length)),description,tone});
 };
 if(bytes.length<8){result.issues.push("报文截断：MBAP头7字节后至少需要1字节功能码");field("MBAP",0,bytes.length,"实际收到的字节");return result;}
 field("MBAP",0,2,`事务标识：${u16(bytes,0)}`,"identity");
 field("MBAP",2,2,`协议标识：${u16(bytes,2)===0?"Modbus":u16(bytes,2)}`);
 field("MBAP",4,2,`后续长度：${u16(bytes,4)} 字节`);
 field("MBAP",6,1,`Unit ID：${bytes[6]}`,"unit");
 if(u16(bytes,2)!==0)result.issues.push("协议标识不是Modbus（应为0）");
 if(u16(bytes,4)<2||u16(bytes,4)>254)result.issues.push("MBAP后续长度应为2～254字节");
 if(u16(bytes,4)+6!==bytes.length)result.issues.push(`MBAP长度不符：声明${u16(bytes,4)+6}字节，实际${bytes.length}字节`);
 const fc=bytes[7]!,base=fc&0x7f,exception=direction==="TX"&&(fc&0x80)!==0;
 result.title=`Modbus TCP · ${exception?"异常响应 · ":""}${functions[exception?base:fc]??`未知功能码 ${fc}`}`;
 field("PDU",7,1,`功能码：${fc}${exception?`（原始功能码 ${base}）`:""} · ${functions[base]??"未知功能码"}`,"function");
 result.subtitle=`Unit ${bytes[6]} · ${direction==="RX"?"请求":"响应"}`;
 if(direction==="RX"&&fc>=128)result.issues.push("请求功能码不能使用异常响应标志位");
 const fail=(message:string)=>{result.issues.push(message);};
 const exact=(length:number)=>{if(bytes.length!==length){fail(`功能码${fc}报文长度应为${length}字节，实际${bytes.length}字节`);return false;}return true;};
 const values=(count:number,label:(index:number)=>string,value:(index:number)=>string)=>{
  result.valueCount=count;result.values=Array.from({length:Math.min(count,32)},(_,index)=>({label:label(index),value:value(index)}));
 };
 const range=(area:string,address:number,quantity:number)=>`${area}[${address}${quantity>1?`–${address+quantity-1}`:""}]`;
 const addressQuantity=(limit:number)=>{
  if(bytes.length<12){fail("PDU截断：缺少起始地址或数量");return;}
  const address=u16(bytes,8),quantity=u16(bytes,10);
  field("PDU",8,2,`起始地址：${address}（零基）`,"address");field("PDU",10,2,`数量：${quantity}`,"address");
  if(quantity<1||quantity>limit)fail(`数量应为1～${limit}`);
  if(address+quantity>65536)fail("地址范围超过65535");
  return {address,quantity};
 };
 if(exception){
  if(exact(9)){const code=bytes[8]!;field("PDU",8,1,`异常码：${code} · ${exceptions[code]??"未知异常码"}`,"value");result.subtitle=`Unit ${bytes[6]} · ${exceptions[code]??`未知异常码 ${code}`}`;}
 }else if(!functions[fc]){
  if(bytes.length>8)field("PDU",8,bytes.length-8,"未解析载荷");
  result.note="当前展示解析仅覆盖FC01/02/03/04/05/06/15/16；保留原始字节。";
 }else if(fc<=4){
  const area=({1:"coil",2:"discrete",3:"holding",4:"input"} as Record<number,string>)[fc]!;
  if(direction==="RX"){
   const span=addressQuantity(fc<3?2000:125);exact(12);
   if(span)result.subtitle=`Unit ${bytes[6]} · ${range(area,span.address,span.quantity)}`;
  }else{
   if(bytes.length<9)fail("读响应截断：缺少字节数");
   else{
    const count=bytes[8]!;field("PDU",8,1,`数据长度：${count} 字节`);
    if(count<1||count>250||(fc>=3&&count%2!==0))fail("读响应数据长度非法");
    exact(9+count);
    const context=readContext(bytes,requestHex);
    if(context&&count!==(fc<3?Math.ceil(context.quantity/8):context.quantity*2))fail("响应字节数与对应请求数量不符");
    if(bytes.length===9+count){
     field("PDU",9,count,fc<3?"位数据（低位在前）":"寄存器数据（原始u16，大端）","value");
     const quantity=context?.quantity??(fc<3?count*8:count/2);
     if(context)result.subtitle=`Unit ${bytes[6]} · ${range(area,context.address,quantity)}`;
     else result.note=fc<3?"未匹配对应请求：地址和实际位数量未知，按数据位展示，末字节可能包含填充位。":"未匹配对应请求：地址未知，按寄存器顺序展示原始u16。";
     values(quantity,index=>context?String(context.address+index):`${fc<3?"位":"寄存器"} ${index}`,index=>fc<3?String(!!(bytes[9+(index>>3)]!&(1<<(index%8)))):String(u16(bytes,9+index*2)));
     if(fc<3&&context&&context.quantity%8&&bytes.at(-1)!>>(context.quantity%8))fail("响应末字节未使用的填充位应为0");
    }
   }
  }
 }else if(fc===5||fc===6){
  if(exact(12)){
   const address=u16(bytes,8),raw=u16(bytes,10),coil=fc===5,value=coil?String(raw===0xff00):String(raw);
   field("PDU",8,2,`地址：${address}（零基）`,"address");field("PDU",10,2,`写入值：${value}${coil?"":"（原始u16）"}`,"value");
   if(coil&&raw!==0&&raw!==0xff00)fail("单线圈写入值只能为FF00（true）或0000（false）");
   result.subtitle=`Unit ${bytes[6]} · ${range(coil?"coil":"holding",address,1)} ← ${value}`;
   values(1,()=>String(address),()=>value);
  }
 }else if(fc===15||fc===16){
  const coil=fc===15,span=addressQuantity(coil?1968:123);
  if(span){
   result.subtitle=`Unit ${bytes[6]} · ${range(coil?"coil":"holding",span.address,span.quantity)}${direction==="TX"?" · 写入确认":""}`;
   if(direction==="TX"){exact(12);result.note="响应仅确认地址与数量，不携带写入值；不代表远端设备动作完成。";}
   else if(bytes.length<13)fail("写请求截断：缺少数据字节数");
   else{
    const count=bytes[12]!;field("PDU",12,1,`数据长度：${count} 字节`);
    if(count!==(coil?Math.ceil(span.quantity/8):span.quantity*2))fail("数据字节数与写入数量不符");
    exact(13+count);
    if(bytes.length===13+count){
     field("PDU",13,count,coil?"写入位数据（低位在前）":"写入寄存器数据（原始u16，大端）","value");
     // 必须先核对数量，避免用非法长度读取缺失值。
     if(count===(coil?Math.ceil(span.quantity/8):span.quantity*2))values(span.quantity,index=>String(span.address+index),index=>coil?String(!!(bytes[13+(index>>3)]!&(1<<(index%8)))):String(u16(bytes,13+index*2)));
     if(coil&&span.quantity%8&&bytes.at(-1)!>>(span.quantity%8))fail("末字节未使用的填充位应为0");
     result.note=coil?`低位在前 · 末字节${(8-span.quantity%8)%8}位为填充`:"仅展示原始u16；多寄存器类型、字序和倍率由模型定义。";
    }
   }
  }
 }
 if(result.issues.length){result.values=[];result.valueCount=0;result.subtitle="报文存在解析问题，请核对原始字节";}
 return result;
}
