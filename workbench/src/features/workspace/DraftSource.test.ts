import { expect, it } from "vitest";
import { DraftSource } from "./DraftSource";
import { ModelDraft } from "./pointDraft";

it("网格重建期间重订阅不会再次收到正在派发的 reset", () => {
  const source = new DraftSource(
    new ModelDraft("m", "M", "1", []),
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
  const draft = new ModelDraft("m", "M", "1", [
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
  sorted.updateCell?.(index, "note", "批量备注");
  source.flush();
  expect(draft.rows[1]?.name).toBe("B");
  expect(draft.rows[0]?.name).toBe("Z");
  expect(draft.revision).toBe(1);
  draft.undo();
  expect(draft.rows[1]?.name).toBe("A");
});
