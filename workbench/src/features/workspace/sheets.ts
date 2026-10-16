import { SnapshotDataSource } from "./SnapshotDataSource";
/** 复用已有NovaSheet数据源；草稿按稳定单元身份和四遥分类隔离。 */
import {
  denseGridTheme,
  type InMemoryDataSource,
} from "@zhiguang/novasheet-core";
import type {
  Category,
  CollectionModel,
  UnitValues,
  Role,
} from "../../shared/api/contracts";
export const readonlyTypes = {
  readonly: {
    editable: false,
    serializeClipboard: (value: unknown) =>
      value == null ? "" : String(value),
  },
};
export const sheetTheme = {
  ...denseGridTheme,
  metrics: {
    ...denseGridTheme.metrics,
    rowHeaderWidth: 38,
    rowHeight: 28,
    headerHeight: 31,
    fontSize: 12,
    fontFamily:
      'system-ui, -apple-system, BlinkMacSystemFont, "PingFang SC", "Microsoft YaHei", sans-serif',
  },
  colors: {
    ...denseGridTheme.colors,
    background: "#ffffff",
    headerBackground: "#f6f8fb",
    headerText: "#33405b",
    text: "#283650",
    gridLine: "#e8edf4",
    gridLineStrong: "#dde5ef",
    selectionBorder: "#0875ff",
    // DOM选区浮层覆盖在Canvas文字上方，必须保留透明度。
    selectionBg: "rgba(8, 117, 255, 0.09)",
    readonlyCellText: "#34415b",
  },
  cell: { ...denseGridTheme.cell, headerTextAlign: "center" as const },
  // 工程工作台自行展示空类说明，不使用组件默认插画及重复文案。
  emptyState: {
    ...denseGridTheme.emptyState,
    layers: [],
    title: "",
    subtitle: "",
  },
};
export const categories: [Category, string][] = [
  ["measurement", "遥测"],
  ["indication", "遥信"],
  ["control", "遥控"],
  ["adjustment", "遥调"],
];
export class SheetViews {
  private views = new Map<string, InMemoryDataSource>();
  get(
    identity: string,
    category: Category,
    model: CollectionModel | undefined,
    values?: UnitValues,
    role: Role = "slave",
  ) {
    // 已应用版本变化时新建视图，普通查询／分类切换不覆盖草稿。
    const key = JSON.stringify([
      identity,
      category,
      model?.id,
      model?.version,
      role,
    ]);
    const acquisition = category === "measurement" || category === "indication";
    const editable = role === "slave" ? acquisition : !acquisition;
    let data = this.views.get(key);
    if (!data) {
      data = new SnapshotDataSource({
        schema: {
          fields: [
            { id: "name", name: "点名", type: "readonly", width: 140 },
            { id: "area", name: "数据区", type: "readonly", width: 90 },
            { id: "address", name: "零基地址", type: "readonly", width: 80 },
            { id: "valueType", name: "类型", type: "readonly", width: 80 },
            { id: "scale", name: "倍率", type: "readonly", width: 65 },
            { id: "unit", name: "单位", type: "readonly", width: 65 },
            {
              id: "draft",
              name:
                role === "slave"
                  ? acquisition
                    ? "置数草稿"
                    : "控制观测值"
                  : acquisition
                    ? "采集结果"
                    : "待发值",
              type: editable ? "text" : "readonly",
              width: 110,
            },
            {
              id: "confirmed",
              name: role === "master" ? "最近采集／下发确认" : "当前应用值",
              type: "readonly",
              width: 110,
            },
          ],
        },
        rows: (model?.points ?? [])
          .filter((point) => point.category === category)
          .map((point) => ({
            id: point.id,
            name: point.name,
            area: point.area,
            address: point.address,
            valueType: point.valueType,
            scale: point.scale,
            unit: point.unit,
            draft: editable
              ? role === "master"
                ? ""
                : String((values?.raw[point.id] ?? point.initial).value)
              : values?.raw[point.id]
                ? String(values.raw[point.id].value)
                : null,
            confirmed: values?.raw[point.id]
              ? String(values.raw[point.id].value)
              : null,
          })),
      });
      this.views.set(key, data);
    }
    // 真实应用确认只更新只读列，不覆盖用户尚未提交的运行值草稿。
    for (const [index, point] of (model?.points ?? [])
      .filter((point) => point.category === category)
      .entries()) {
      const confirmed = values?.raw[point.id]
        ? String(values.raw[point.id].value)
        : null;
      if (data.getRows(index, index)[0]?.confirmed !== confirmed)
        data.updateCell(index, "confirmed", confirmed);
      if (!editable && data.getCell(index, "draft") !== confirmed)
        data.updateCell(index, "draft", confirmed);
    }
    return data;
  }
}
