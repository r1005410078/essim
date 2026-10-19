import {expect,it} from "vitest";
import type {SlaveView} from "../../shared/api/contracts";
import {SlaveResponseGate} from "./slaveResponses";
it("置数后旧查询不能覆盖新值，重启后的新运行可重新从小修订号开始",()=>{
 const gate=new SlaveResponseGate();const beforeWrite=gate.ticket();
 const view=(revision:string,run="a")=>({recordPath:run,revision} as SlaveView);
 gate.invalidate();expect(gate.accept(beforeWrite,view("9"))).toBe(false);
 const current=gate.ticket();expect(gate.accept(current,view("12"))).toBe(true);
 expect(gate.accept(current,view("11"))).toBe(false);
 gate.invalidate();expect(gate.accept(gate.ticket(),view("1","b"))).toBe(true);
 expect(gate.accept(current,view("15","a"))).toBe(false);
});
