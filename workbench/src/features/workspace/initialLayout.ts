/** 仅在创建布局时分配首屏比例，后续窗口变化不覆盖用户拖动结果。 */
export function initialWorkbenchLayout(width: number, height: number) {
  const fit = (value: number, minimum: number, maximum: number) =>
    Math.max(minimum, Math.min(maximum, Math.round(value / 4) * 4));
  return {
    navigation: fit(width * 0.18, 180, 248),
    inspector: fit(width * 0.22, 220, 300),
    output: fit(height * 0.26, 208, 240),
  };
}
