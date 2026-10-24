import {expect,it,vi} from 'vitest';
import {menuKeyboard} from './menuKeyboard';
it('菜单方向键首尾循环，Tab和Esc返回树，Enter保留原生按钮行为',()=>{
 const items=[{focus:vi.fn()},{focus:vi.fn()},{focus:vi.fn()}];const close=vi.fn();
 const root={querySelectorAll:()=>items} as unknown as HTMLElement;
 const event=(key:string,target:unknown)=>({key,target,currentTarget:root,preventDefault:vi.fn(),stopPropagation:vi.fn()}) as unknown as React.KeyboardEvent<HTMLElement>;
 menuKeyboard(event('ArrowDown',items[2]),close);expect(items[0]!.focus).toHaveBeenCalledOnce();
 menuKeyboard(event('ArrowUp',items[0]),close);expect(items[2]!.focus).toHaveBeenCalledOnce();
 menuKeyboard(event('End',items[0]),close);expect(items[2]!.focus).toHaveBeenCalledTimes(2);
 menuKeyboard(event('Escape',items[1]),close);menuKeyboard(event('Tab',items[1]),close);expect(close).toHaveBeenCalledTimes(2);
 menuKeyboard(event('Enter',items[1]),close);expect(close).toHaveBeenCalledTimes(2);
});
