import { describe, expect, it } from 'vitest';
import { constrainDialog, resizeDialog } from '@essim/ui/lib/dialog-geometry';
import type {DialogEdge} from '@essim/ui/lib/dialog-geometry';
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

// 明确验证八向拖动时对边锚点，避免只测右下角而漏掉上／左方向。
it.each<[DialogEdge,number,number,number,number]>([
 ['n',100,130,500,370],['s',100,100,500,430],
 ['e',100,100,520,400],['w',120,100,480,400],
 ['ne',100,130,520,370],['nw',120,130,480,370],
 ['se',100,100,520,430],['sw',120,100,480,430],
])('八向缩放 %s 保持未拖动边的位置',(edge,x,y,width,height)=>{
 expect(resizeDialog({x:100,y:100,width:500,height:400},edge,20,30,viewport)).toEqual({x,y,width,height});
});
