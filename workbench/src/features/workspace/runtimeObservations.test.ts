import {describe,it,expect} from "vitest";
import {mergeObservations,mergeObservationWindow,observationRows} from "./runtimeObservations";
import type {SlaveObservation} from "../../shared/api/contracts";
const entry=(n:number,event:object):SlaveObservation=>({sequence:String(n),unixMs:"1792300000000",event:JSON.stringify(event)});
describe("真实运行观测",()=>{
 it("重复查询去重，保留精确序号并限制缓存",()=>{
  const rows=Array.from({length:600},(_,i)=>entry(i,{kind:"listening"}));
  const merged=mergeObservations(rows.slice(0,300),rows.slice(250));
  expect(merged).toHaveLength(500);expect(merged[0]?.sequence).toBe("100");
 });
 it("仅匹配同一客户端的完整发送确认，准备状态不能作为成功",()=>{
  const rows=[entry(1,{kind:"responsePrepared",client:1,bytes:[0,1,0,0,0,3,1,3,0]}),entry(2,{kind:"responsePrepared",client:2,bytes:[0,2]}),entry(3,{kind:"responseSent",client:2})];
  const messages=observationRows(rows).filter(r=>r.direction!=="事件");
  expect(messages.map(r=>r.status)).toEqual(["待发送","已写出socket"]);
  expect(messages[0]?.transaction).toBe(1);
  expect(observationRows(rows)[2]?.summary).toBe("响应已完整写出socket");
 });
 it("无准备记录的发送确认不伪造字节，坏记录安全呈现",()=>{
  const rows=observationRows([entry(1,{kind:"responseSent",client:1}),{sequence:"2",unixMs:"0",event:"bad"}]);
  expect(rows.every(r=>r.direction==="事件")).toBe(true);
  expect(rows[1]?.summary).toBe("无法解析的观测记录");
 });
});

it("停止后新运行序号重用会清除旧缓存，恢复展示则保留同一运行",()=>{
 const previous={run:"first",records:[entry(1,{kind:"listening"}),entry(2,{kind:"stopped"})]};
 expect(mergeObservationWindow(previous,"first",[entry(3,{kind:"connected"})]).records).toHaveLength(3);
 const next=mergeObservationWindow(previous,"second",[entry(1,{kind:"listening"})]);
 expect(next.records).toHaveLength(1);expect(next.run).toBe("second");
});

it("只关联同连接、事务号、Unit和功能码的唯一请求，事务复用重新关联",()=>{
 const req=[0,1,0,0,0,6,1,3,0,10,0,1],res=[0,1,0,0,0,5,1,3,2,0,7];
 const rows=observationRows([
  entry(1,{kind:"request",client:1,bytes:req}),entry(2,{kind:"responsePrepared",client:2,bytes:res}),
  entry(3,{kind:"responsePrepared",client:1,bytes:res}),entry(4,{kind:"request",client:1,bytes:[...req.slice(0,9),20,0,1]}),entry(5,{kind:"responsePrepared",client:1,bytes:res}),
 ]);
 expect(rows[1]?.requestHex).toBeUndefined();expect(rows[2]?.requestHex).toContain("00 0A");expect(rows[4]?.requestHex).toContain("00 14");
});
it("同事务并发重复或不匹配的Unit不会借用地址",()=>{
 const req=[0,1,0,0,0,6,1,3,0,10,0,1],res=[0,1,0,0,0,5,1,3,2,0,7];
 const rows=observationRows([entry(1,{kind:"request",client:1,bytes:req}),entry(2,{kind:"request",client:1,bytes:req}),entry(3,{kind:"responsePrepared",client:1,bytes:res}),entry(4,{kind:"responsePrepared",client:1,bytes:res}),entry(5,{kind:"responsePrepared",client:1,bytes:[...res.slice(0,6),2,...res.slice(7)]})]);
 expect(rows.slice(2).every(row=>row.requestHex===undefined)).toBe(true);
});

it("主站TX请求关联RX响应且保留实际方向",()=>{
 const records=[{sequence:"1",unixMs:"1",event:JSON.stringify({kind:"frame",direction:"TX",hex:"00 01 00 00 00 06 01 03 00 0A 00 01"})},{sequence:"2",unixMs:"2",event:JSON.stringify({kind:"frame",direction:"RX",hex:"00 01 00 00 00 05 01 03 02 00 07"})}];
 const rows=observationRows(records,true);expect(rows[0].direction).toBe("TX");expect(rows[1].direction).toBe("RX");expect(rows[1].requestHex).toBe(rows[0].hex);
});

it("RTU从站保留实际方向，准备与驱动写出分开且仅准备响应关联地址",()=>{
 const frame=(direction:string,hex:string,stage:string)=>({kind:"rtuFrame",direction,hex,stage,complete:true,crcValid:true,discarded:false});
 const rows=observationRows([entry(1,frame("RX","01 03 00 0A 00 01 A4 08","received")),entry(2,frame("TX","01 03 02 00 07 F9 86","prepared")),entry(3,frame("TX","01 03 02 00 07 F9 86","driverWritten"))]);
 expect(rows.map(row=>row.direction)).toEqual(["RX","TX","TX"]);
 expect(rows[1].requestHex).toBe(rows[0].hex);expect(rows[2].requestHex).toBeUndefined();
 expect(rows[1].status).toContain("发送准备");expect(rows[2].status).toContain("本机驱动");expect(rows.every(row=>row.transport==="rtu")).toBe(true);
});
