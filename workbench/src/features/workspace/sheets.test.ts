import { expect, it } from "vitest";
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
