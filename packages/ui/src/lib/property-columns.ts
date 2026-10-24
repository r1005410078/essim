/** 名称列允许向左缩至零；右侧仍保留值列的可用宽度。 */
export function propertyColumnRatio(pixels: number, panelWidth: number): number {
  if (panelWidth <= 0) return 0.36;
  const minimum = 0;
  const maximum = Math.max(panelWidth * 0.45, panelWidth - 120);
  return Math.min(maximum, Math.max(minimum, pixels)) / panelWidth;
}
