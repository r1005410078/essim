import { expect, it } from "vitest";
import { describeUnit, filterUnits } from "./unitOptions";
const units = [
  {id:"a",station:1,name:"Unit 1",enabled:true},
  {id:"b",station:2,name:"电池簇 B",enabled:false},
  {id:"c",station:12,name:"单元 12",enabled:true},
];
it("默认名称合并，自定义名称保留，站号不替代稳定ID", () => {
  expect(describeUnit(units[0]!)).toMatchObject({number:"01",name:null,label:"Unit 01"});
  expect(describeUnit(units[1]!)).toMatchObject({number:"02",name:"电池簇 B"});
  expect(describeUnit(units[2]!).name).toBeNull();
  expect(describeUnit({...units[0]!,name:"Unit 12"}).name).toBe("Unit 12");
});
it("按名称与线上站号搜索，保留顺序、禁用状态和稳定身份", () => {
  expect(filterUnits(units,"  电池簇 b ").map(u=>u.id)).toEqual(["b"]);
  expect(filterUnits(units,"02").map(u=>u.id)).toEqual(["b"]);
  expect(filterUnits(units,"12").map(u=>u.id)).toEqual(["c"]);
  expect(filterUnits(units,"")).toEqual(units);
  expect(filterUnits(units,"missing")).toEqual([]);
});
