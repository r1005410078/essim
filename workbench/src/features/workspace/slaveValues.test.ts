import {expect,it} from "vitest";
import {masterRaw} from "./masterWrites";
import {slaveRaw} from "./slaveValues";
import type {PointDefinition} from "../../shared/api/contracts";
const p={id:"p",name:"P",valueType:"u64"} as PointDefinition;
it("运行置数保持64位整数文本，拒绝小数及空输入",()=>{
 expect(slaveRaw(p,"18446744073709551615")).toEqual({kind:"unsigned",value:"18446744073709551615"});
 expect(()=>slaveRaw(p,"1.1")).toThrow();expect(()=>slaveRaw(p,"")).toThrow();
});
it("Bool和浮点按原始值契约输入，不静默转换非法值",()=>{
 expect(slaveRaw({...p,valueType:"bool"},"0")).toEqual({kind:"bool",value:false});
 expect(()=>slaveRaw({...p,valueType:"bool"},"2")).toThrow();
 expect(()=>slaveRaw({...p,valueType:"f32"},"Infinity")).toThrow();
 expect(slaveRaw({...p,valueType:"f64"},"-0.125")).toEqual({kind:"float",value:"-0.125"});
});

it("主站f32待写小数按目标精度编码而不丢失64位整数",()=>{
 expect(masterRaw({...p,valueType:"f32"},"1.1")).toEqual({kind:"float",value:String(Math.fround(1.1))});
 expect(()=>masterRaw({...p,valueType:"f32"},"1e100")).toThrow();
 expect(masterRaw(p,"18446744073709551615")).toEqual({kind:"unsigned",value:"18446744073709551615"});
});
