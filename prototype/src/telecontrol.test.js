import test from "node:test";
import assert from "node:assert/strict";
import {
  normalizeGroup,
  pointCategory,
  validateAction,
  findPeerPoint,
} from "./telecontrol.js";
test("迁移旧角色与四遥分类不丢失已有点", () => {
  const g = normalizeGroup({
    role: "客户端 / 主站",
    points: [
      { id: "a", name: "运行状态", type: "UInt16" },
      { id: "b", name: "有功功率" },
    ],
  });
  assert.equal(g.role, "主站");
  assert.deepEqual(g.points.map(pointCategory), ["遥信", "遥测"]);
  assert.equal(g.points.length, 2);
});
test("主从站允许的操作不同，遥信遥控仅允许0或1", () => {
  assert.equal(validateAction("从站", "遥测", "置数", 32), "");
  assert.ok(validateAction("主站", "遥测", "置数", 32));
  assert.equal(validateAction("主站", "遥调", "发送", 120), "");
  assert.ok(validateAction("从站", "遥调", "发送", 120));
  assert.equal(validateAction("从站", "遥控", "接收", 1), "");
  assert.ok(validateAction("从站", "遥信", "置数", 2));
  assert.ok(validateAction("主站", "遥调", "发送", NaN));
});
test("本地对端按四遥和地址唯一匹配，歧义不发送", () => {
  const p = { category: "遥测", address: "100" };
  assert.equal(findPeerPoint(p, [{ ...p, id: "x" }]).id, "x");
  assert.throws(
    () =>
      findPeerPoint(p, [
        { ...p, id: "x" },
        { ...p, id: "y" },
      ]),
    /重复/,
  );
  assert.throws(
    () => findPeerPoint(p, [{ category: "遥信", address: "100" }]),
    /找不到/,
  );
});
