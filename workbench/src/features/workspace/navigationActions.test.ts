import { describe, expect, it, vi } from "vitest";
import { canAddUnit, editNavigationUnit, simulatorActions } from "./navigationActions";
import { WorkspaceStore, projectNavigation } from "./model";
import type { WorkspaceClient, WorkspaceView } from "../../shared/api/contracts";

function setup() {
  const execute = vi.fn();
  const store = new WorkspaceStore({ execute } as unknown as WorkspaceClient);
  store.receive({ sequence: "1", catalog: {
    models: [{ id: "m", name: "点表", points: [] }], devices: [], applied: [], values: {},
    endpoints: [{ id: "e", name: "模拟器", role: "slave", transport: { kind: "tcp", address: "127.0.0.1", port: 1502 },
      units: [1, 3].map(station => ({ id: `u${station}`, station, name: `Unit ${station}`, enabled: true, model: { id: "m" }, device: null })) }],
  }, capabilities: { configuration: true, execution: false, recording: false, replay: false, sessionOnly: true, reason: "" } } satisfies WorkspaceView);
  return { store, execute };
}
const target = { kind: "endpoint", id: "e", unit: "u1" } as const;

describe("树菜单按稳定身份操作", () => {
  it("Unit菜单只提供新增、重命名和删除单元；模拟器保留整机操作", () => {
    expect(simulatorActions(target)).toEqual(["newUnit", "renameUnit", "deleteUnit"]);
    expect(simulatorActions({ kind: "endpoint", id: "e" })).toContain("copy");
    expect(simulatorActions({ kind: "folder", id: "f" })).not.toContain("newUnit");
  });
  it("重命名只修改指定Unit，保留模拟器、兄弟单元和保存配置", () => {
    const { store, execute } = setup();
    store.edit({ ...store.endpoint("e")!, note: "已有草稿" });
    editNavigationUnit(store, "renameUnit", target, { name: "电池簇" });
    expect(store.endpoint("e")?.name).toBe("模拟器");
    expect(store.endpoint("e")?.note).toBe("已有草稿");
    expect(store.endpoint("e")?.units.map(u => u.name)).toEqual(["电池簇", "Unit 3"]);
    expect(store.view!.catalog.endpoints[0]!.units[0]!.name).toBe("Unit 1");
    expect(execute).not.toHaveBeenCalled();
  });
  it("删除指定Unit并切换到剩余单元，不删除模拟器或应用快照", () => {
    const { store, execute } = setup();
    store.view!.catalog.applied = structuredClone(store.view!.catalog.endpoints);
    store.selectUnit("e", "u1");
    editNavigationUnit(store, "deleteUnit", target);
    expect(store.endpoint("e")?.units.map(u => u.id)).toEqual(["u3"]);
    expect(store.selectedUnits.get("e")).toBe("u3");
    expect(store.view!.catalog.applied[0]!.units).toHaveLength(2);
    expect(store.view!.catalog.endpoints).toHaveLength(1);
    expect(execute).not.toHaveBeenCalled();
    store.discardEndpoint("e");
    expect(store.endpoint("e")?.units).toHaveLength(2);
  });
  it("新增Unit使用空闲站号、独立身份和选定模型，不覆盖已有草稿", () => {
    const { store } = setup();
    const id = editNavigationUnit(store, "newUnit", target, { name: "新单元", station: 2, model: "m" });
    expect(store.endpoint("e")?.units.at(-1)).toMatchObject({ id, station: 2, name: "新单元", model: { id: "m" } });
    expect(["u1", "u3"]).not.toContain(id);
    expect(store.selectedUnits.get("e")).toBe(id);
    const tree = projectNavigation({ ...store.view!.catalog, endpoints: [store.endpoint("e")!] }, "simulators");
    expect([...tree.targets.values()]).toContainEqual({ kind: "endpoint", id: "e", unit: id });
  });
  it("拒绝失效Unit、重复站号、缺失模型和RTU从站第二个Unit，失败不改草稿", () => {
    const { store } = setup();
    const before = structuredClone(store.endpoint("e"));
    expect(() => editNavigationUnit(store, "deleteUnit", { ...target, unit: "missing" })).toThrow();
    expect(() => editNavigationUnit(store, "renameUnit", { kind: "endpoint", id: "e" }, { name: "整机？" })).toThrow();
    for (const values of [{ name: "新", station: 1, model: "m" }, { name: "新", station: 2, model: "missing" }])
      expect(() => editNavigationUnit(store, "newUnit", target, values)).toThrow();
    expect(store.endpoint("e")).toEqual(before);
    store.edit({ ...before!, transport: { kind: "rtu", path: "test", baud: 9600, parity: "none", dataBits: 8, stopBits: 1 } });
    expect(canAddUnit(store.endpoint("e")!)).toBe(false);
    expect(() => editNavigationUnit(store, "newUnit", target, { name: "新", station: 2, model: "m" })).toThrow();
  });
  it("删除非当前单元保留选择，删除最后一个单元仍保留空模拟器", () => {
    const { store, execute } = setup();
    store.selectUnit("e", "u3");
    editNavigationUnit(store, "deleteUnit", target);
    expect(store.selectedUnits.get("e")).toBe("u3");
    editNavigationUnit(store, "deleteUnit", { ...target, unit: "u3" });
    expect(store.endpoint("e")).toMatchObject({ id: "e", name: "模拟器", units: [] });
    expect(store.selectedUnits.has("e")).toBe(false);
    expect(canAddUnit(store.endpoint("e")!)).toBe(true);
    expect(execute).not.toHaveBeenCalled();
  });
});
