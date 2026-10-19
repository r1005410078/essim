import {decodeModbusMessage,messageHex,type DecodedMessage} from "./modbusMessage";
export function rtuCrc(bytes:readonly number[]){let crc=0xffff;for(const b of bytes){crc^=b;for(let i=0;i<8;i++)crc=crc&1?(crc>>>1)^0xa001:crc>>>1;}return crc;}
/** 仅复用PDU展示逻辑；虚拟头不进入观测、字节展示或执行路径。 */
export function decodeRtuMessage(hex:string,direction:"RX"|"TX",requestHex?:string):DecodedMessage{
 const parse=(text:string)=>text.trim().split(/\s+/).map(b=>/^[0-9a-f]{2}$/i.test(b)?Number.parseInt(b,16):NaN);
 const bytes=parse(hex);const empty:DecodedMessage={title:"Modbus RTU",subtitle:"",fields:[],issues:[],values:[],valueCount:0,note:""};
 if(bytes.length<4||bytes.length>256||bytes.some(Number.isNaN)){empty.issues.push("RTU帧不完整、格式错误或超过256字节；仅保留实际字节");return empty;}
 const wireCrc=rtuCrc(bytes)===0;const toPdu=(data:number[])=>messageHex([0,0,0,0,0,data.length-2,...data.slice(0,-2)]);
 const fc=bytes[1]!;let result:DecodedMessage;
 if([1,2,3,4,5,6,15,16].includes(fc&0x7f)){
  const req=requestHex?parse(requestHex):undefined;
  result=decodeModbusMessage(toPdu(bytes),direction,req&&req.length>=4&&rtuCrc(req)===0?toPdu(req):undefined);
  result.title=result.title.replace("TCP","RTU");result.fields=result.fields.filter(f=>f.section==="PDU").map(f=>({...f,offset:f.offset-6}));
 }else{
  const names:Record<number,string>={7:"读取异常状态",8:"串行诊断",11:"通信事件计数",12:"通信事件日志",17:"服务器标识"};
  result={...empty,title:`Modbus RTU · ${names[fc&0x7f]??"其他功能"}`,subtitle:`Unit ${bytes[0]} · ${direction==="RX"?"请求":"响应"}`,fields:[{section:"PDU",offset:1,hex:messageHex([fc]),tone:"function",description:`功能码：${fc&0x7f}`}],note:"诊断含义取决于远端实现；未取得的数据不推测。"};
  if(fc&0x80){if(bytes.length!==5)result.issues.push("异常响应长度应为5字节");result.fields.push({section:"PDU",offset:2,hex:messageHex(bytes.slice(2,-2)),tone:"value",description:`异常码：${bytes[2]}`});}
  else if(fc===8&&bytes.length===8){result.fields.push({section:"PDU",offset:2,hex:messageHex(bytes.slice(2,4)),tone:"address",description:`子功能：0x${messageHex(bytes.slice(2,4)).replace(/ /g,"")}`},{section:"PDU",offset:4,hex:messageHex(bytes.slice(4,6)),tone:"value",description:`诊断／回送数据：${(bytes[4]!<<8)|bytes[5]!}`});}
  else if(fc===11&&bytes.length===8){for(const [offset,label]of [[2,"通信状态"],[4,"通信事件计数"]] as const)result.fields.push({section:"PDU",offset,hex:messageHex(bytes.slice(offset,offset+2)),tone:"value",description:`${label}：${(bytes[offset]!<<8)|bytes[offset+1]!}`});}
  else if(fc===12&&bytes.length>=11){for(const [offset,label]of [[3,"通信状态"],[5,"事件计数"],[7,"消息计数"]] as const)result.fields.push({section:"PDU",offset,hex:messageHex(bytes.slice(offset,offset+2)),tone:"value",description:`${label}：${(bytes[offset]!<<8)|bytes[offset+1]!}`});if(bytes.length>11)result.fields.push({section:"PDU",offset:9,hex:messageHex(bytes.slice(9,-2)),tone:"value",description:"通信事件字节（含义参见标准与设备资料）"});}
  else if(fc===17&&bytes.length>=7){result.fields.push({section:"PDU",offset:2,hex:messageHex(bytes.slice(2,5)),tone:"value",description:`数据字节数：${bytes[2]}；服务器ID：${bytes[3]}；运行标志：0x${bytes[4]!.toString(16).padStart(2,"0")}`});if(bytes.length>7)result.fields.push({section:"PDU",offset:5,hex:messageHex(bytes.slice(5,-2)),tone:"value",description:"设备附加标识（原始字节）"});}
  else if(bytes.length>4)result.fields.push({section:"PDU",offset:2,hex:messageHex(bytes.slice(2,-2)),tone:"value",description:fc===7?`异常状态位：0x${bytes[2]!.toString(16).padStart(2,"0")}`:"实际诊断载荷（包括协议定义的状态／计数或标识字段）"});
 }
 result.fields.unshift({section:"RTU",offset:0,hex:messageHex([bytes[0]!]),tone:"unit",description:`站号：${bytes[0]}`});
 result.fields.push({section:"RTU",offset:bytes.length-2,hex:messageHex(bytes.slice(-2)),tone:"neutral",description:`CRC16（低字节在前）：${wireCrc?"正确":"错误"}`});
 if(!wireCrc)result.issues.push("CRC错误，不能作为有效响应");return result;
}
