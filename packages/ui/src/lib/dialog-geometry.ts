export interface DialogRect { x: number; y: number; width: number; height: number }
export interface DialogViewport { width: number; height: number }
export type DialogEdge = 'n' | 's' | 'e' | 'w' | 'ne' | 'nw' | 'se' | 'sw';
const margin = 8;
const clamp = (value: number, min: number, max: number) => Math.max(min, Math.min(value, max));
/** 保证标题、内容和操作栏均在当前视口内。极小视口优先保证可见。 */
export function constrainDialog(rect: DialogRect, viewport: DialogViewport): DialogRect {
  const width = Math.min(Math.max(320, rect.width), Math.max(1, viewport.width - margin * 2));
  const height = Math.min(Math.max(160, rect.height), Math.max(1, viewport.height - margin * 2));
  return { width, height, x: clamp(rect.x, margin, viewport.width - width - margin), y: clamp(rect.y, margin, viewport.height - height - margin) };
}
/** 以未拖动的对边为锚点，限制最小尺寸与视口边界。 */
export function resizeDialog(rect: DialogRect, edge: DialogEdge, dx: number, dy: number, viewport: DialogViewport): DialogRect {
  let { x, y, width, height } = rect;
  const minWidth = Math.min(320, viewport.width - margin * 2);
  const minHeight = Math.min(160, viewport.height - margin * 2);
  if (edge.includes('e')) width = clamp(width + dx, minWidth, viewport.width - x - margin);
  if (edge.includes('s')) height = clamp(height + dy, minHeight, viewport.height - y - margin);
  if (edge.includes('w')) { const right = x + width; x = clamp(x + dx, margin, right - minWidth); width = right - x; }
  if (edge.includes('n')) { const bottom = y + height; y = clamp(y + dy, margin, bottom - minHeight); height = bottom - y; }
  return { x, y, width, height };
}
