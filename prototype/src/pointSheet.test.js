import test from "node:test";
import assert from "node:assert/strict";
import { InMemoryDataSource } from "@zhiguang/novasheet-core";
import {
  POINT_SHEET_SPARE_ROWS,
  ensureSpareRows,
  isBlankPointRow,
  planSpareRows,
} from "./pointSheet.js";

const fields = [
  { id: "name", name: "点名称", type: "text" },
  { id: "address", name: "地址", type: "text" },
  { id: "scale", name: "倍率", type: "number" },
  { id: "value", name: "初始值", type: "number" },
];

function point(name) {
  return { name, address: "1", scale: 1, value: 0 };
}

test("100 条有效数据扩展为 1100 行", () => {
  const rows = Array.from({ length: 100 }, (_, i) => point(`点${i}`));
  const plan = planSpareRows(rows);
  assert.equal(POINT_SHEET_SPARE_ROWS, 1000);
  assert.equal(plan.desired, 1100);
  assert.equal(plan.insert, 1000);
  assert.deepEqual(plan.deleteIds, []);
});

test("末尾空行保持 1000，中间空行保留", () => {
  const rows = [point("甲"), {}, point("乙"), {}, {}];
  const plan = planSpareRows(rows, 2);
  assert.equal(plan.desired, 5);
  assert.equal(plan.insert, 0);
  assert.deepEqual(plan.deleteIds, []);
  assert.equal(isBlankPointRow({}), true);
  assert.equal(isBlankPointRow({ name: "  " }), true);
  assert.equal(isBlankPointRow({ scale: 0 }), false);
});

test("写入空行后继续补足余量，删掉有效行后裁掉多余空行", () => {
  const data = new InMemoryDataSource({
    schema: { fields },
    rows: [point("有功功率")],
  });
  ensureSpareRows(data, 3);
  assert.equal(data.getRowCount(), 4);
  data.updateCell(3, "name", "新点");
  ensureSpareRows(data, 3);
  assert.equal(data.getRowCount(), 7);
  assert.equal(data.getCell(3, "name"), "新点");
  data.updateCell(3, "name", "");
  data.updateCell(0, "name", "");
  data.updateCell(0, "address", "");
  data.updateCell(0, "scale", "");
  data.updateCell(0, "value", "");
  ensureSpareRows(data, 3);
  assert.equal(data.getRowCount(), 3);
});
