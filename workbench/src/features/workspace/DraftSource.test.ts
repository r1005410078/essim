import { expect, it } from "vitest";
import { DraftSource } from "./DraftSource";
import { ModelDraft, defaultRow } from "./pointDraft";

it("网格重建期间重订阅不会再次收到正在派发的 reset", () => {
  const source = new DraftSource(
    new ModelDraft("m", "M", []),
    "measurement",
  );
  let calls = 0;
  let cancel = () => {};
  const listener = () => {
    if (++calls > 6) throw Error("同一通知发生循环重入");
    cancel();
    cancel = source.subscribe(listener);
  };
  cancel = source.subscribe(listener);
  source.sync();
  expect(calls).toBeLessThanOrEqual(2);
});

it("排序后的写入仍指向稳定点身份，一次粘贴只产生一个草稿事务", async () => {
  const { defaultRow } = await import("./pointDraft");
  const { SortLayer, isMutableDataSource } = await import(
    "@zhiguang/novasheet-core"
  );
  const draft = new ModelDraft("m", "M", [
    { ...defaultRow("measurement", "z"), name: "Z" },
    { ...defaultRow("measurement", "a"), name: "A" },
  ]);
  const source = new DraftSource(draft, "measurement");
  const layer = new SortLayer();
  layer.setSpec({ fieldId: "name", direction: "asc" });
  const sorted = layer.wrap(source);
  if (!isMutableDataSource(sorted)) throw Error("排序视图不可写");
  const index = (await sorted.getRows(0, sorted.getRowCount() - 1)).findIndex(
    (r) => r.id === "a",
  );
  sorted.updateCell?.(index, "name", "B");
  sorted.updateCell?.(index, "unit", "V");
  source.flush();
  expect(draft.rows[1]?.name).toBe("B");
  expect(draft.rows[0]?.name).toBe("Z");
  expect(draft.editCount).toBe(1);
  draft.undo();
  expect(draft.rows[1]?.name).toBe("A");
});


it("修改分类后原位保留当前行，重新打开分类才重新过滤", () => {
  const row = { ...defaultRow("measurement", "p"), name: "SOC", address: "100" };
  const next = defaultRow("measurement", "q");
  const draft = new ModelDraft("m", "M", [row, next]);
  const source = new DraftSource(draft, "measurement");
  draft.change(state => { state.rows[0]!.category = "indication"; });
  source.sync();
  expect(source.getRows(0, 1).map(r => r.id)).toEqual(["p", "q"]);
  expect(source.getRows(0, 0)[0]).toMatchObject({ ...row, category: "indication" });
  expect(new DraftSource(draft, "measurement").getRows(0, 0)[0]?.id).toBe("q");
  expect(new DraftSource(draft, "indication").getRows(0, 0)[0]?.id).toBe("p");
  draft.undo();
  source.sync();
  expect(source.getRows(0, 0)[0]).toMatchObject(row);
});

it("在中间空行选下拉后保持原行位置，后续预览同步不重排", () => {
  const draft = new ModelDraft("m", "M", []);
  const source = new DraftSource(draft, "measurement");
  const ids = source.getRows(0, 5).map(r => r.id);
  source.updateCell(4, "area", "input");
  source.flush();
  source.sync();
  expect(source.getRows(0, 5).map(r => r.id)).toEqual(ids);
  expect(source.getRows(4, 4)[0]).toMatchObject({ id: ids[4], name: "新点", area: "input" });
  draft.undo();
  source.sync();
  expect(draft.rows).toHaveLength(0);
});
