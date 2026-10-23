import { expect, it } from "vitest";
import { SortLayer } from "@zhiguang/novasheet-core";
import { ModelDraft, defaultRow, gridFields } from "./pointDraft";
import { DraftSource } from "./DraftSource";
import { pastePointText } from "./pointClipboard";
const selection = (col = 0) => ({
  activeCell: { rowIndex: 0, colIndex: col },
  anchorCell: null,
  extentCell: null,
  selectedRange: { startRow: 0, endRow: 0, startCol: col, endCol: col },
});
it("系统TSV粘贴可扩展空表，500行一次撤销，64位文本不经Number", async () => {
  const draft = new ModelDraft("m", "M", []);
  const source = new DraftSource(draft, "measurement");
  await pastePointText(
    source,
    () => source,
    selection(),
    Array.from({ length: 500 }, (_, i) => `点${i}`).join("\n"),
  );
  expect(draft.rows).toHaveLength(500);
  expect(new Set(draft.rows.map((r) => r.id)).size).toBe(500);
  expect(draft.editCount).toBe(1);
  draft.undo();
  source.sync();
  expect(draft.rows).toHaveLength(0);
  await pastePointText(
    source,
    () => source,
    selection(gridFields.indexOf("initialEngineering")),
    "18446744073709551615",
  );
  expect(draft.rows[0]!.initialEngineering).toBe("18446744073709551615");
});
it("排序键在粘贴中变化仍写入原目标身份，非适用字段跳过", async () => {
  const draft = new ModelDraft("m", "M", [
    { ...defaultRow("measurement", "z"), name: "Z" },
    { ...defaultRow("measurement", "a"), name: "A" },
  ]);
  const source = new DraftSource(draft, "measurement");
  const sort = new SortLayer();
  sort.setSpec({ fieldId: "name", direction: "asc" });
  const view = sort.wrap(source);
  // 排序中的空占位排在前面；先在非空目标处定位，不假定行0就是第一条点。
  const rows = await view.getRows(0, view.getRowCount() - 1);
  const start = rows.findIndex((r) => r.id === "a");
  const target = {
    ...selection(),
    activeCell: { rowIndex: start, colIndex: 0 },
    selectedRange: { startRow: start, endRow: start, startCol: 0, endCol: 0 },
  };
  await pastePointText(source, () => view, target, "ZZ\nAA");
  expect(draft.rows.find((r) => r.id === "a")?.name).toBe("ZZ");
  expect(draft.rows.find((r) => r.id === "z")?.name).toBe("AA");
  draft.undo();
  source.sync();
  const skipped = await pastePointText(
    source,
    () => source,
    selection(gridFields.indexOf("wordOrder")),
    "little",
  );
  expect(skipped).toBe(1);
  expect(draft.rows[0]!.wordOrder).toBe("big");
});

it("四遥分页粘贴按当前分类归属，点名后直接对应数据区", async () => {
  for (const category of ["measurement", "indication", "control", "adjustment"] as const) {
    const draft = new ModelDraft("m", "M", []);
    const source = new DraftSource(draft, category);
    await pastePointText(source, () => source, selection(), "测试点\tholding\t123");
    expect(draft.rows[0]).toMatchObject({ name: "测试点", category, area: "holding", address: "123" });
  }
});
