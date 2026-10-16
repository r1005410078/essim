import { expect, it } from "vitest";
import { ModelDraft, defaultRow } from "./pointDraft";
import { appendValidated } from "./pointImport";
import type { DraftPreview } from "../../shared/api/contracts";
const valid = {
  model: { id: "m", name: "M", version: "1", points: [] },
  errors: [],
  previews: [],
} satisfies DraftPreview;
it("追加失败与取消不改变草稿和撤销；成功一次撤销全部追加", async () => {
  const draft = new ModelDraft("m", "M", "1", [
    defaultRow("measurement", "old"),
  ]);
  const rows = [defaultRow("measurement", "new")];
  const before = draft.content();
  await expect(
    appendValidated(
      draft,
      rows,
      async () => ({
        ...valid,
        model: null,
        errors: [{ code: "ADDRESS_OVERLAP", message: "地址重叠" }],
      }),
      () => true,
    ),
  ).rejects.toMatchObject({ code: "ADDRESS_OVERLAP" });
  expect(draft.content()).toBe(before);
  expect(draft.canUndo).toBe(false);
  expect(
    await appendValidated(
      draft,
      rows,
      async () => valid,
      () => false,
    ),
  ).toBe(false);
  expect(draft.content()).toBe(before);
  await appendValidated(
    draft,
    rows,
    async () => valid,
    () => true,
  );
  expect(draft.rows.map((row) => row.id)).toEqual(["old", "new"]);
  draft.undo();
  expect(draft.content()).toBe(before);
});
it("预览期间目标变化时不使用旧校验追加", async () => {
  const draft = new ModelDraft("m", "M", "1", []);
  let complete!: (value: DraftPreview) => void;
  const task = appendValidated(
    draft,
    [defaultRow("measurement", "new")],
    () =>
      new Promise((resolve) => {
        complete = resolve;
      }),
    () => true,
  );
  draft.change((state) => {
    state.name = "另一次编辑";
  });
  complete(valid);
  await expect(task).rejects.toThrow("草稿已变化");
  expect(draft.rows).toHaveLength(0);
  draft.undo();
  expect(draft.name).toBe("M");
});
