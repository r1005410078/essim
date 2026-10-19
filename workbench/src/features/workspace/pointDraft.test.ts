import { describe, expect, it } from "vitest";
import { ModelDraft, defaultRow } from "./pointDraft";

describe("模型唯一草稿事务", () => {
  it("新建点默认允许外部写入", () => {
    expect(defaultRow("control", "new").slaveWritable).toBe("true");
  });
  it("重复视图选中通知不会触发重复刷新或修改草稿", () => {
    let notifications = 0;
    const draft = new ModelDraft(
      "m",
      "M",
      [defaultRow("measurement", "a")],
      () => notifications++,
    );
    draft.select(null);
    draft.select("a");
    draft.select("a");
    expect(notifications).toBe(1);
    expect(draft.dirty).toBe(false);
  });
  it("分类切换保留身份；批量操作可一次撤销", () => {
    const draft = new ModelDraft("m", "M", [], undefined, false);
    const a = defaultRow("measurement", "a");
    const b = defaultRow("indication", "b");
    draft.change((s) => {
      s.rows.push(a, b);
    });
    draft.change((s) => {
      s.rows[0]!.category = "indication";
      s.rows[1]!.name = "B";
    });
    expect(
      draft.rows.filter((r) => r.category === "indication").map((r) => r.id),
    ).toEqual(["a", "b"]);
    draft.undo();
    expect(draft.rows[0]!.category).toBe("measurement");
    expect(draft.rows[1]!.name).toBe("新点");
    draft.redo();
    expect(draft.rows[1]!.name).toBe("B");
  });
  it("保存期间的新编辑不被较早保存确认清除", () => {
    const draft = new ModelDraft("m", "M", [
      defaultRow("measurement", "a"),
    ]);
    draft.change((s) => {
      s.rows[0]!.name = "保存的值";
    });
    const submitted = draft.content();
    draft.change((s) => {
      s.rows[0]!.name = "后续修改";
    });
    draft.saved(submitted);
    expect(draft.dirty).toBe(true);
    expect(draft.rows[0]!.name).toBe("后续修改");
    expect(draft.persisted).toBe(true);
  });
  it("导入追加保持旧点且单次撤销，取消不修改", () => {
    const draft = new ModelDraft("m", "M", [
      defaultRow("measurement", "a"),
    ]);
    const before = draft.content();
    draft.change((s) => s.rows.push(defaultRow("control", "new")));
    expect(draft.rows[0]!.id).toBe("a");
    draft.undo();
    expect(draft.content()).toBe(before);
    expect(draft.dirty).toBe(false);
  });
});
it("类型变为单地址或Bool时清除不适用编码，同一事务可恢复", () => {
  const row = {
    ...defaultRow("measurement", "p"),
    valueType: "u32",
    wordOrder: "little",
    byteOrder: "little",
    scale: "0.1",
  };
  const draft = new ModelDraft("m", "M", [row]);
  draft.change((s) => {
    s.rows[0]!.valueType = "bool";
  });
  expect(draft.rows[0]).toMatchObject({
    scale: "1",
    wordOrder: "big",
    byteOrder: "big",
  });
  draft.undo();
  expect(draft.rows[0]).toEqual(row);
});
