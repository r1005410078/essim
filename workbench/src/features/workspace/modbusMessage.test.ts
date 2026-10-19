import {describe,it,expect} from "vitest";
import {decodeModbusMessage} from "./modbusMessage";
const hex=(bytes:number[])=>bytes.map(b=>b.toString(16).padStart(2,"0")).join(" ");
const frame=(pdu:number[],unit=1,transaction=1)=>hex([transaction>>8,transaction&255,0,0,0,pdu.length+1,unit,...pdu]);
describe("Modbus TCP展示解析",()=>{
 it("FC15四点按低位在前，MBAP完整7字节",()=>{
  const result=decodeModbusMessage(frame([15,3,232,0,4,1,15]),"RX");
  expect(result.issues).toEqual([]);expect(result.title).toContain("写多个线圈");
  expect(result.fields.filter(f=>f.section==="MBAP").map(f=>f.hex).join(" ")).toBe("00 01 00 00 00 08 01");
  expect(result.values).toEqual([1000,1001,1002,1003].map(address=>({label:String(address),value:"true"})));
 });
 it.each([1,2,3,4])("FC%d请求地址数量与读响应分开解析",fc=>{
  const request=frame([fc,0,10,0,2]);
  expect(decodeModbusMessage(request,"RX").subtitle).toContain("10–11");
  const response=frame(fc<3?[fc,1,1]:[fc,4,0,7,0,42]);
  const result=decodeModbusMessage(response,"TX",request);
  expect(result.issues).toEqual([]);
  expect(result.values?.map(v=>v.value)).toEqual(fc<3?["true","false"]:["7","42"]);
  expect(result.values?.map(v=>v.label)).toEqual(["10","11"]);
 });
 it.each([[5,255,0,"true"],[6,0,42,"42"]])("FC%d单写请求及回显",(fc,hi,lo,value)=>{
  for(const direction of ["RX","TX"] as const){const result=decodeModbusMessage(frame([fc as number,0,10,hi as number,lo as number]),direction);expect(result.issues).toEqual([]);expect(result.values?.[0]?.value).toBe(value);}
 });
 it("FC16多寄存器请求保留原始u16，响应仅确认地址数量",()=>{
  const req=frame([16,0,10,0,2,4,0,7,0,42]);
  expect(decodeModbusMessage(req,"RX").values?.map(v=>v.value)).toEqual(["7","42"]);
  const res=decodeModbusMessage(frame([16,0,10,0,2]),"TX",req);
  expect(res.issues).toEqual([]);expect(res.values).toEqual([]);expect(res.subtitle).toContain("10–11");
 });
 it("FC15响应没有写入数据，异常响应解释原始功能码",()=>{
  expect(decodeModbusMessage(frame([15,3,232,0,4]),"TX").values).toEqual([]);
  const result=decodeModbusMessage(frame([0x8f,2]),"TX");
  expect(result.title).toContain("异常响应");expect(result.fields.some(f=>f.description.includes("非法数据地址"))).toBe(true);
 });
 it("缺请求不能猜地址、coil数量或填充位",()=>{
  const result=decodeModbusMessage(frame([1,1,15]),"TX");
  expect(result.note).toContain("未匹配");expect(result.values?.[0]?.label).toBe("位 0");
  expect(result.values).toHaveLength(8);
 });
 it("错误连接之外，解码器也拒绝错事务、Unit和功能码上下文",()=>{
  const response=frame([3,2,0,7]);
  for(const request of [frame([3,0,10,0,1],2),frame([3,0,10,0,1],1,2),frame([4,0,10,0,1])])expect(decodeModbusMessage(response,"TX",request).note).toContain("未匹配");
 });
 it("畸形MBAP、截断、额外字节、非法数量和值不冒充有效结果",()=>{
  const bad=["00 GG", "00 01", "00 01 00 01 00 02 01 03",frame([3,0]),frame([3,0,0,0,0]),frame([5,0,0,0,1]),frame([15,0,0,0,9,1,255]),frame([16,0,0,0,2,3,0,1,0]),frame([3,3,0,1,2]),frame([0x83]),frame([3,0,0,0,1])+" 00"];
  for(const input of bad){const direction=input===frame([3,3,0,1,2])||input===frame([0x83])?"TX":"RX";const result=decodeModbusMessage(input,direction);expect(result.issues.length,input).toBeGreaterThan(0);expect(result.values,input).toEqual([]);}
 });
 it("响应数量不符、读地址溢出、非零填充位明示问题",()=>{
  expect(decodeModbusMessage(frame([3,2,0,1]),"TX",frame([3,0,0,0,2])).issues.length).toBeGreaterThan(0);
  expect(decodeModbusMessage(frame([3,255,255,0,2]),"RX").issues.length).toBeGreaterThan(0);
  expect(decodeModbusMessage(frame([15,0,0,0,1,1,255]),"RX").issues.length).toBeGreaterThan(0);
 });
 it("未知功能码保留原始载荷，不硬套字段",()=>{
  const result=decodeModbusMessage(frame([43,14,1,0]),"RX");expect(result.title).toContain("未知功能码");expect(result.fields.at(-1)?.hex).toBe("0E 01 00");expect(result.values).toEqual([]);
 });
 it("长响应预览有界，保留实际总数",()=>{
  const result=decodeModbusMessage(frame([1,250,...Array(250).fill(255)]),"TX");expect(result.values).toHaveLength(32);expect(result.valueCount).toBe(2000);
 });
});
