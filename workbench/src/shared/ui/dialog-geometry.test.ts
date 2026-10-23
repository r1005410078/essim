import { describe, expect, it } from 'vitest';
import { constrainDialog, resizeDialog } from '@essim/ui/lib/dialog-geometry';
const viewport = { width: 1000, height: 700 };
describe('弹框边界与缩放', () => {
  it('窗口缩小时保留整个弹框与操作栏', () => {
    expect(constrainDialog({ x: 900, y: 650, width: 600, height: 500 }, viewport)).toEqual({ x: 392, y: 192, width: 600, height: 500 });
    expect(constrainDialog({ x: -20, y: -20, width: 600, height: 500 }, { width: 280, height: 200 })).toEqual({ x: 8, y: 8, width: 264, height: 184 });
  });
  it('左上角缩放固定右下角，到达最小尺寸后不漂移', () => {
    expect(resizeDialog({ x: 100, y: 100, width: 500, height: 400 }, 'nw', 800, 700, viewport)).toEqual({ x: 280, y: 340, width: 320, height: 160 });
  });
  it('向视口外缩放会停在边界，反向缩放不移动另一条边', () => {
    expect(resizeDialog({ x: 100, y: 100, width: 500, height: 400 }, 'se', 900, 900, viewport)).toEqual({ x: 100, y: 100, width: 892, height: 592 });
    expect(resizeDialog({ x: 100, y: 100, width: 500, height: 400 }, 'w', -900, 0, viewport)).toEqual({ x: 8, y: 100, width: 592, height: 400 });
  });
});
