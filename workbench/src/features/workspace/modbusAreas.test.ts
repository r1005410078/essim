import { expect, it } from "vitest";
import { serializeRowsToTsv } from "@zhiguang/novasheet-core";
import { areaHelp, formatModbusArea, modbusAreas } from "./modbusAreas";
import { DraftSource } from "./DraftSource";
import { defaultRow, ModelDraft } from "./pointDraft";
import { pastePointText } from "./pointClipboard";

it("中文展示不改变复制、粘贴及提交中的协议标识", async () => {
  const draft = new ModelDraft("m", "模型", [defaultRow("measurement", "p")]);
  const source = new DraftSource(draft, "measurement");
  const schema = source.getSchema();
  const field = schema.fields.find(field => field.id === "area")!;
  const context = { field, locale: "zh-CN" };
  expect(field.format).toEqual({ kind: "custom", formatterId: "modbusArea" });
  expect(formatModbusArea(source.getCell(0, "area")!, context)).toBe("保持寄存器");
  expect(serializeRowsToTsv(source.getRows(0, 0), ["area"], schema)).toBe("holding");
  const col = schema.fields.indexOf(field);
  await pastePointText(source, () => source, {
    activeCell: { rowIndex: 0, colIndex: col }, anchorCell: null, extentCell: null,
    selectedRange: { startRow: 0, endRow: 0, startCol: col, endCol: col },
  }, "input");
  expect(draft.input().points[0]!.area).toBe("input");
  expect(formatModbusArea(source.getCell(0, "area")!, context)).toBe("输入寄存器");
  draft.undo();
  expect(draft.rows[0]!.area).toBe("holding");
  expect(formatModbusArea("invalid-area", context)).toBe("invalid-area");
  expect(formatModbusArea(null, context)).toBe("");
});

it("每个数据区选项都有独立 Markdown 说明", () => {
  for (const area of modbusAreas) {
    expect(areaHelp[area.id]).toContain(`### ${area.name}`);
    expect(areaHelp[area.id]).toContain(area.description);
    expect(areaHelp[area.id]).toContain("功能码");
  }
});
