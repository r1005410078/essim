import type { CellFormatter } from "@zhiguang/novasheet-core";
import content from "./modbusAreas.md?raw";

/** 数据区标识用于协议与文件交换，中文名称仅用于展示。 */
export const modbusAreas = [
  { id: "holding", name: "保持寄存器", description: "16 位，读写" },
  { id: "input", name: "输入寄存器", description: "16 位，只读" },
  { id: "coil", name: "线圈", description: "1 位，读写" },
  { id: "discrete", name: "离散输入", description: "1 位，只读" },
] as const;
export function modbusArea(value: string) {
  return modbusAreas.find(area => area.id === value);
}
export const areaHelp = Object.fromEntries(content.split(/^## /m).slice(1).map(section => {
  const newline = section.indexOf("\n");
  return [section.slice(0, newline).trim(), section.slice(newline + 1)];
}));
/** 未知旧值原样展示，避免把非法配置伪装成合法数据区。 */
export const formatModbusArea: CellFormatter = value => modbusArea(String(value ?? ""))?.name ?? String(value ?? "");
