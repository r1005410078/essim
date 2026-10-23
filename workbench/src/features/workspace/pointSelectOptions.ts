import type { CellFormatter } from "@zhiguang/novasheet-core";
import { areaHelp, modbusAreas } from "./modbusAreas";
import content from "./pointSelectOptions.md?raw";

export interface PointSelectOption {
  id: string;
  name: string;
  description: string;
  markdown: string;
  source: string;
}
const help = Object.fromEntries(content.split(/^## /m).slice(1).map(section => {
  const newline = section.indexOf("\n");
  return [section.slice(0, newline).trim(), section.slice(newline + 1)];
}));
const options: Record<string, readonly PointSelectOption[]> = {
  area: modbusAreas.map(area => ({...area, markdown:areaHelp[area.id] ?? "", source:"功能码为十进制 · Modbus V1.1b3 §4.3、§6"})),
  valueType: ([
    ["bool", "布尔量", "1 位，1 个位地址"],
    ["i16", "16 位有符号整数", "16 位，1 个寄存器"],
    ["u16", "16 位无符号整数", "16 位，1 个寄存器"],
    ["i32", "32 位有符号整数", "32 位，2 个连续寄存器"],
    ["u32", "32 位无符号整数", "32 位，2 个连续寄存器"],
    ["i64", "64 位有符号整数", "64 位，4 个连续寄存器"],
    ["u64", "64 位无符号整数", "64 位，4 个连续寄存器"],
    ["f32", "32 位单精度浮点数", "32 位，2 个连续寄存器"],
    ["f64", "64 位双精度浮点数", "64 位，4 个连续寄存器"],
  ] as const).map(([id, name, description]) => ({id, name, description, markdown:help[`valueType.${id}`] ?? "", source:"ESSim 点表类型映射 · 浮点采用 IEEE 754"})),
  byteOrder: ([
    ["big", "大端序", "每个16位字内，高字节在前"],
    ["little", "小端序", "每个16位字内，低字节在前"],
  ] as const).map(([id, name, description]) => ({id, name, description, markdown:help[`byteOrder.${id}`] ?? "", source:"ESSim 应用数值映射 · 以设备手册为准"})),
  wordOrder: ([
    ["big", "高字在前", "多寄存器从高位字到低位字"],
    ["little", "低字在前", "多寄存器从低位字到高位字"],
  ] as const).map(([id, name, description]) => ({id, name, description, markdown:help[`wordOrder.${id}`] ?? "", source:"ESSim 多寄存器排列约定 · 以设备手册为准"})),
};
/** 只处理当前点表的四个枚举字段；其他下拉保留原有展示。 */
export function hasPointSelectOptions(field: string) {
  return Object.hasOwn(options, field);
}
export function pointSelectOption(field: string, value: string) {
  return options[field]?.find(option => option.id === value);
}
export function pointSelectLabel(field: string, value: string) {
  if (field === "valueType" || field === "byteOrder" || field === "wordOrder") return value;
  return pointSelectOption(field, value)?.name ?? value;
}
/** 中文仅用于绘制；未知旧值保留原文，剪贴板和保存继续使用原标识。 */
export const formatPointOption: CellFormatter = (value, context) => pointSelectLabel(context.field.id, String(value ?? ""));
