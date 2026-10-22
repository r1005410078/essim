/** 将期望名称列宽限制在两列可用范围内；极窄面板按45%／55%退让。 */
export function propertyColumnRatio(pixels: number, panelWidth: number): number {
  if (panelWidth <= 0) return 0.36;
  const minimum = Math.min(112, panelWidth * 0.45);
  const maximum = Math.max(panelWidth * 0.45, panelWidth - 120);
  return Math.min(maximum, Math.max(minimum, pixels)) / panelWidth;
}
