import { expect, it } from "vitest";
import { initialWorkbenchLayout } from "./initialLayout";

it("首屏在笔记本上让中央文档区保留过半宽度", () => {
  const layout = initialWorkbenchLayout(1264, 761);
  expect(1264 - layout.navigation - layout.inspector - 12).toBeGreaterThan(700);
  expect(layout.output).toBeGreaterThanOrEqual(208);
  expect(761 - layout.output).toBeGreaterThan(500);
});

it("小窗口收窄边栏，大窗口不会让边栏和空输出区无限增长", () => {
  const small = initialWorkbenchLayout(884, 561);
  const large = initialWorkbenchLayout(2544, 1400);
  expect(small.navigation).toBe(180);
  expect(small.inspector).toBe(220);
  expect(884 - small.navigation - small.inspector - 12).toBeGreaterThan(460);
  expect(large.navigation).toBeLessThanOrEqual(248);
  expect(large.inspector).toBeLessThanOrEqual(300);
  expect(large.output).toBeLessThanOrEqual(240);
});
