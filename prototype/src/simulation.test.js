import test from "node:test";
import assert from "node:assert/strict";
import { simulate } from "./simulation.js";
test("执行脚本并根据功率更新SOC，功率为正表示放电", () => {
  const result = simulate(
    'function on_tick(ctx){ctx.command("PCS_01.目标功率",100)}',
    { capacity: 1000, soc: 50, lower: 20, upper: 90, maxPower: 250 },
  );
  assert.ok(result.samples[1].soc < 50);
  assert.ok(result.samples.every((s) => s.soc >= 20));
});
test("脚本异常和无效指令不能伪装成成功", () => {
  assert.throws(
    () => simulate('function on_tick(){throw new Error("测试失败")}', {}),
    /测试失败/,
  );
  assert.throws(
    () => simulate('function on_tick(ctx){ctx.command("错误点",100)}', {}),
    /未绑定/,
  );
});
test("限功率、SOC边界和参数校验", () => {
  const r = simulate(
    'function on_tick(ctx){ctx.command("PCS_01.目标功率",-999)}',
    { upper: 80, soc: 79, maxPower: 100 },
  );
  assert.ok(r.samples.every((p) => p.soc <= 80 && Math.abs(p.power) <= 100));
  assert.throws(() => simulate("", { capacity: 0 }), /容量/);
});
