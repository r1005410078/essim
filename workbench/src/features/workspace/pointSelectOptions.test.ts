import { expect, it } from "vitest";
import { serializeRowsToTsv } from "@zhiguang/novasheet-core";
import { pointSelectOption, formatPointOption } from "./pointSelectOptions";
import { choices, defaultRow, ModelDraft } from "./pointDraft";
import { DraftSource } from "./DraftSource";
import { pastePointText } from "./pointClipboard";

it("每个现有类型和排列选项都有中文、属性和独立Markdown说明", () => {
  for (const field of ["valueType", "byteOrder", "wordOrder"]) {
    for (const id of choices[field]!) {
      const option = pointSelectOption(field, id)!;
      expect(option.name).toMatch(/[\u4e00-\u9fff]/);
      expect(option.description).toBeTruthy();
      expect(option.markdown).toContain(`### ${option.name}`);
    }
  }
  expect(pointSelectOption("byteOrder", "little")?.markdown).toContain("34 12");
  expect(pointSelectOption("wordOrder", "little")?.markdown).toContain("7788、5566、3344、1122");
  expect(pointSelectOption("wordOrder", "big")?.name).not.toBe(pointSelectOption("byteOrder", "big")?.name);
});

it("中文格式化后复制粘贴与提交仍使用协议值，排序字段不串用", async () => {
  const draft = new ModelDraft("m", "模型", [{...defaultRow("measurement", "p"), valueType:"i32"}]);
  const source = new DraftSource(draft, "measurement");
  const schema = source.getSchema();
  for (const [id, raw, label] of [["valueType", "f64", "f64"], ["byteOrder", "little", "little"], ["wordOrder", "little", "little"]]) {
    const field = schema.fields.find(field => field.id === id)!;
    expect(field.format).toEqual({ kind: "custom", formatterId: "pointOption" });
    const col = schema.fields.indexOf(field);
    await pastePointText(source, () => source, {
      activeCell:{rowIndex:0,colIndex:col},anchorCell:null,extentCell:null,
      selectedRange:{startRow:0,endRow:0,startCol:col,endCol:col},
    }, raw!);
    expect(draft.input().points[0]![id!]).toBe(raw);
    expect(serializeRowsToTsv(source.getRows(0,0), [id!], schema)).toBe(raw);
    expect(formatPointOption(raw!, {field, locale:"zh-CN"})).toBe(label);
    expect(formatPointOption("unknown", {field, locale:"zh-CN"})).toBe("unknown");
    expect(formatPointOption(null, {field, locale:"zh-CN"})).toBe("");
  }
  draft.undo();
  expect(draft.rows[0]!.wordOrder).toBe("big");
});

it("显示中文不会解除Bool和单寄存器的排列字段保护", () => {
  const draft = new ModelDraft("m", "模型", [{...defaultRow("measurement", "p"),valueType:"i32",byteOrder:"little",wordOrder:"little"}]);
  const source = new DraftSource(draft, "measurement");
  source.updateCell(0,"valueType","u16");source.flush();
  expect(draft.rows[0]!.wordOrder).toBe("big");
  expect(source.canEdit(0,"wordOrder")).toBe(false);
  source.updateCell(0,"valueType","bool");source.flush();
  expect(draft.rows[0]).toMatchObject({byteOrder:"big",wordOrder:"big",scale:"1"});
  expect(source.canEdit(0,"byteOrder")).toBe(false);
  expect(source.canEdit(0,"wordOrder")).toBe(false);
});
