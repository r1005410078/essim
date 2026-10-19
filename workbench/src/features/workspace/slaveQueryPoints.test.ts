import {expect,it} from "vitest";
import {runtimeVisiblePoints,resetRangePoints} from "./slaveQueryPoints";
it("查询跟随真实可见行与排序映射，不读取选区；冻结区域去重",()=>{
 const ids=["c","a","b","d"];
 expect(runtimeVisiblePoints([{rowRange:[1,2]},{rowRange:[1,2]}],row=>ids[row])).toEqual(["a","b"]);
 expect(runtimeVisiblePoints([{rowRange:[3,3]}],row=>ids[row])).toEqual(["d"]);
});
it("空视口不请求点值，并保持查询有界",()=>{
 expect(runtimeVisiblePoints([],()=>"p")).toEqual([]);
 expect(runtimeVisiblePoints([{rowRange:[0,2000]}],row=>String(row))).toHaveLength(1024);
});
it("地址范围复位只匹配指定数据区，拒绝空洞和截断多寄存器点",()=>{
 const points=[{id:"a",area:"holding",address:10,valueType:"u32"},{id:"b",area:"coil",address:10,valueType:"bool"}];
 expect(resetRangePoints(points,"holding",10,11)).toEqual(["a"]);
 expect(resetRangePoints(points,"coil",10,10)).toEqual(["b"]);
 expect(()=>resetRangePoints(points,"holding",11,11)).toThrow();
 expect(()=>resetRangePoints(points,"holding",10,12)).toThrow();
});
