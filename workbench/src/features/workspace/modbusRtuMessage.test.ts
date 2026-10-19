import {it,expect} from "vitest";
import {decodeRtuMessage,rtuCrc} from "./modbusRtuMessage";
const adu=(bytes:number[])=>{const crc=rtuCrc(bytes);return [...bytes,crc&255,crc>>>8].map(n=>n.toString(16).padStart(2,"0")).join(" ");};
it("RTU请求展示站号、零基地址与CRC，不显示虚拟MBAP",()=>{
 const parsed=decodeRtuMessage("01 03 00 0A 00 02 E4 09","RX");expect(parsed.issues).toEqual([]);expect(parsed.fields.some(f=>f.section==="MBAP")).toBe(false);expect(parsed.fields.some(f=>f.description.includes("CRC16")&&f.description.includes("正确"))).toBe(true);expect(parsed.subtitle).toContain("10");
});
it("RTU实际响应仅在取得有效请求时补充地址",()=>{
 const response=adu([1,3,4,0,7,0,42]);const parsed=decodeRtuMessage(response,"TX","01 03 00 0A 00 02 E4 09");expect(parsed.values.map(v=>v.value)).toEqual(["7","42"]);expect(parsed.issues).toEqual([]);
 expect(decodeRtuMessage("01 03 04 00 07 00 2A 00 00","TX").issues).toContain("CRC错误，不能作为有效响应");expect(decodeRtuMessage("01 03","TX").issues[0]).toContain("不完整");
});
it("诊断08显示真实子功能和回送值",()=>{const parsed=decodeRtuMessage(adu([1,8,0,0,0x12,0x34]),"TX");expect(parsed.fields.some(f=>f.description.includes("4660"))).toBe(true);expect(parsed.title).toContain("诊断");});
