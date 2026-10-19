import { expect, it } from "vitest";
import {acknowledgeMasterWrites} from "./masterWrites";
import type {MasterView} from "../../shared/api/contracts";
import { resolveUnitSheet, SheetViews } from "./sheets";
import type { Catalog, CollectionModel, EndpointConfig } from "../../shared/api/contracts";

const model: CollectionModel = { id: "m", name: "电表", points: [{
  id: "p", name: "电压", category: "measurement", area: "holding", address: 100,
  valueType: "u16", byteOrder: "big", wordOrder: "big", scale: "1", unit: "V", note: "",
  initialEngineering: "220", initial: { kind: "unsigned", value: "220" }, slaveWritable: false, masterPeriodMs: 1000,
}] };
const endpoint: EndpointConfig = { id: "e", name: "E", role: "slave", transport: { kind: "tcp", address: "127.0.0.1", port: 1502 },
  units: [{ id: "u", name: "U", station: 1, enabled: true, model: { id: "m" }, device: null }] };
const fixture = (): Catalog => ({ models: [structuredClone(model)], endpoints: [structuredClone(endpoint)], applied: [structuredClone(endpoint)], devices: [],
  values: { u: { model: { ...structuredClone(model), points: [] }, raw: {}, stale: false } } });

it("上次应用空模型后保存新点，模拟器立即展示已保存点表而非旧空快照", () => {
  const catalog = fixture();
  const before = structuredClone(catalog.values);
  const resolved = resolveUnitSheet(catalog, endpoint, endpoint.units[0]!);
  const data = new SheetViews().get("u", "measurement", resolved.model, resolved.values);
  expect(data.getRowCount()).toBe(1);
  expect(data.getRows(0, 0)[0]).toMatchObject({ id: "p", name: "电压", address: 100, draft: "220", confirmed: null });
  expect(catalog.values).toEqual(before);
});

it("更换引用立即展示新模型，不把原模型值套到新模型同名点ID", () => {
  const catalog = fixture();
  catalog.values.u = { model: structuredClone(model), raw: { p: { kind: "unsigned", value: "111" } }, stale: false };
  const next = { ...structuredClone(model), id: "new", name: "新模型" };
  next.points[0]!.name = "新点";
  catalog.models.push(next);
  const unit = { ...endpoint.units[0]!, model: { id: next.id } };
  const resolved = resolveUnitSheet(catalog, endpoint, unit);
  expect(resolved.model).toBe(next);
  expect(resolved.values).toBeUndefined();
});

it("尚未应用也能预览引用点表；应用内容一致才展示确认值", () => {
  const catalog = fixture();
  catalog.applied = [];
  expect(resolveUnitSheet(catalog, endpoint, endpoint.units[0]!).model).toEqual(model);
  catalog.applied = [structuredClone(endpoint)];
  catalog.values.u = { model: structuredClone(model), raw: { p: { kind: "unsigned", value: "222" } }, stale: false };
  const resolved = resolveUnitSheet(catalog, endpoint, endpoint.units[0]!,{e:{u:true}});
  expect(resolved.values?.raw.p).toEqual({ kind: "unsigned", value: "222" });
});

it("展示改名保持视图草稿和选中数据源，仅更新展示列",()=>{
 const views=new SheetViews();const before=views.get("u","measurement",structuredClone(model));before.updateCell(0,"draft","999");
 const renamed=structuredClone(model);renamed.name="改名";renamed.points[0]!.name="新点名";renamed.points[0]!.unit="千伏";
 const after=views.get("u","measurement",renamed);expect(after).toBe(before);expect(after.getCell(0,"draft")).toBe("999");expect(after.getCell(0,"name")).toBe("新点名");
});

it("未编辑不待提交，实际值刷新跟随；修改后保留草稿，成功仅清除提交时的版本",()=>{
 const views=new SheetViews();const values={model,raw:{p:{kind:"unsigned" as const,value:"220"}},stale:false};
 const data=views.get("u","measurement",model,values);
 expect(views.pending("u",model)).toHaveLength(0);
 values.raw.p.value="221";views.get("u","measurement",model,values);
 expect(data.getCell(0,"draft")).toBe("221");
 data.updateCell(0,"draft","999");const sent=views.pending("u",model);
 expect(sent).toHaveLength(1);
 values.raw.p.value="222";views.get("u","measurement",model,values);
 expect(data.getCell(0,"draft")).toBe("999");
 data.updateCell(0,"draft","1000");views.acknowledge(sent);
 expect(views.pending("u",model)[0]?.text).toBe("1000");
 views.acknowledge(views.pending("u",model));expect(views.pending("u",model)).toHaveLength(0);
});
it("待修改按当前Unit和模型归集，不取决于分类或选区；撤销回原值清除待修改",()=>{
 const views=new SheetViews();const mixed=structuredClone(model);
 mixed.points.push({...mixed.points[0]!,id:"b",category:"indication",area:"coil",valueType:"bool",initial:{kind:"bool",value:false}});
 const a=views.get("u","measurement",mixed);const b=views.get("u","indication",mixed);
 a.updateCell(0,"draft","123");b.updateCell(0,"draft","true");
 expect(views.pending("u",mixed).map(item=>item.point.id)).toEqual(["p","b"]);
 expect(views.pending("other",mixed)).toEqual([]);
 a.updateCell(0,"draft","220");expect(views.pending("u",mixed).map(item=>item.point.id)).toEqual(["b"]);
});

it("主站待写值独立于最近读取值，失败结果与更新时间可见",()=>{
 const m=structuredClone(model);m.points[0]!.category="adjustment";const sheets=new SheetViews();
 const values={model:m,raw:{p:{kind:"unsigned" as const,value:"7"}},stale:false};
 const data=sheets.get("u","adjustment",m,values,"master",{p:{value:values.raw.p,unixMs:"1792300000000",quality:"failed",error:"超时",writeResult:"未执行"}});
 data.updateCell(0,"draft","42");
 sheets.get("u","adjustment",m,{...values,raw:{p:{kind:"unsigned",value:"9"}}},"master",{p:{value:values.raw.p,unixMs:"1792300000000",quality:"failed",error:"超时",writeResult:"未执行"}});
 expect(data.getCell(0,"draft")).toBe("42");expect(data.getCell(0,"confirmed")).toBe("9");expect(data.getCell(0,"quality")).toBe("读取失败");expect(data.getCell(0,"result")).toBe("未执行");
 expect(sheets.pendingMaster("u",m).map(e=>e.point.id)).toEqual(["p"]);
 sheets.discardMaster("u");expect(sheets.pendingMaster("u",m)).toEqual([]);
});

it("旧批次成功不会确认新草稿，下发期间继续编辑也不会被清空",()=>{
 const m=structuredClone(model);m.points[0]!.category="adjustment";const sheets=new SheetViews();const data=sheets.get("u","adjustment",m,undefined,"master");data.updateCell(0,"draft","42");const edits=sheets.pendingMaster("u",m);
 const view:MasterView={endpoint:"e",state:"connected",polling:false,pausedUnits:[],busy:false,operation:"new",completed:1,total:1,requests:1,error:null,revision:"1",observations:[],recordPath:null,points:{p:{value:null,unixMs:null,quality:"unread",error:null,writeOperation:"old",writeResult:"协议已确认，回验一致"}}};
 acknowledgeMasterWrites(sheets,"new",edits,view);expect(sheets.pendingMaster("u",m)).toHaveLength(1);
 data.updateCell(0,"draft","43");view.points.p!.writeOperation="new";acknowledgeMasterWrites(sheets,"new",edits,view);expect(sheets.pendingMaster("u",m)[0]?.text).toBe("43");
 data.updateCell(0,"draft","42");expect(sheets.pendingMaster("u",m)).toHaveLength(0);
});
