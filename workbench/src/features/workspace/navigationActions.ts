import type { EndpointConfig } from "../../shared/api/contracts";
import { nextStation, type Target, type WorkspaceStore } from "./model";

export type UnitNavigationAction = "newUnit" | "renameUnit" | "deleteUnit";
export type SimulatorAction = UnitNavigationAction | "newEndpoint" | "newFolder" | "copy" | "rename" | "move" | "delete";

/** Unit与端点共享文档类型，菜单必须再按Unit稳定身份区分作用范围。 */
export function simulatorActions(target?: Target): SimulatorAction[] {
  if (target?.kind === "endpoint" && target.unit) return ["newUnit", "renameUnit", "deleteUnit"];
  if (target?.kind === "endpoint") return ["newEndpoint", "newFolder", "newUnit", "copy", "rename", "move", "delete"];
  if (target?.kind === "folder") return ["newEndpoint", "newFolder", "rename", "move", "delete"];
  return ["newEndpoint", "newFolder"];
}

export function canAddUnit(endpoint: EndpointConfig): boolean {
  return nextStation(endpoint) !== undefined && !(endpoint.transport.kind === "rtu" && endpoint.role === "slave" && endpoint.units.length > 0);
}

/** 单元结构操作仅编辑所属端点草稿，保存和应用沿用完整端点的原子校验。 */
export function editNavigationUnit(store: WorkspaceStore, kind: UnitNavigationAction, target: Target,
  values: { name?: string; station?: number; model?: string } = {}): string | undefined {
  const endpoint = target.kind === "endpoint" ? store.endpoint(target.id) : undefined;
  if (!endpoint) throw Error("模拟器已不存在");
  const unit = endpoint.units.find(u => u.id === target.unit);
  if (kind !== "newUnit" && !unit) throw Error("单元已不存在，请重新选择");
  if (kind === "newUnit") {
    if (!canAddUnit(endpoint)) throw Error("站号已用尽，或RTU从站已包含一个单元");
    const station = values.station ?? nextStation(endpoint)!;
    if (!Number.isInteger(station) || station < 1 || station > 247 || endpoint.units.some(u => u.station === station)) throw Error("站号必须为1至247之间的未使用整数");
    if (!store.view?.catalog.models.some(m => m.id === values.model)) throw Error("请选择有效的采集模型");
    if (!values.name?.trim()) throw Error("单元名称不能为空");
    const id = crypto.randomUUID();
    store.edit({ ...endpoint, units: [...endpoint.units, { id, name: values.name.trim(), station, enabled: true, model: { id: values.model! }, device: null }] });
    store.selectUnit(endpoint.id, id);
    return id;
  }
  if (kind === "renameUnit") {
    if (!values.name?.trim()) throw Error("单元名称不能为空");
    store.edit({ ...endpoint, units: endpoint.units.map(u => u.id === unit!.id ? { ...u, name: values.name!.trim() } : u) });
    return unit!.id;
  }
  const units = endpoint.units.filter(u => u.id !== unit!.id);
  // 确认删除同时丢弃该Unit的点表草稿；其他Unit及运行快照保留。
  store.unitDrafts.delete(unit!.id);
  store.runtimePoint.delete(unit!.id);
  if (store.selectedUnits.get(endpoint.id) === unit!.id) {
    if (units[0]) store.selectedUnits.set(endpoint.id, units[0].id);
    else store.selectedUnits.delete(endpoint.id);
  }
  store.edit({ ...endpoint, units });
  return store.selectedUnits.get(endpoint.id);
}
