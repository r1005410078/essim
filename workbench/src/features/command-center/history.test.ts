import {expect,it} from 'vitest';
import {NavigationHistory} from './history';
it('后退前进不增加记录，分支访问覆盖前进项，删除项跳过',()=>{
 const h=new NavigationHistory<string>(x=>x,3);h.visit('a');h.visit('b');h.visit('c');
 expect(h.move(-1)).toBe('b');expect(h.move(-1)).toBe('a');expect(h.move(1)).toBe('b');h.visit('d');expect(h.canMove(1)).toBe(false);
 expect(h.move(-1,x=>x!=='b')).toBe('a');expect(h.move(1,x=>x!=='b')).toBe('d');
});
it('连续相同访问去重，上限裁剪后仍保持正确位置',()=>{const h=new NavigationHistory<string>(x=>x,2);h.visit('a');h.visit('a');expect(h.canMove(-1)).toBe(false);h.visit('b');h.visit('c');expect(h.move(-1)).toBe('b');expect(h.move(-1)).toBeNull();});
