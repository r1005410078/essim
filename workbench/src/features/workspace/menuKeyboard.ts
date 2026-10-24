import type {KeyboardEvent} from 'react';
/** 自定义树菜单只管理导航键；Enter／Space沿用原生按钮点击语义。 */
export function menuKeyboard(event:KeyboardEvent<HTMLElement>,close:()=>void){
 if(['Escape','Tab'].includes(event.key)){event.preventDefault();event.stopPropagation();close();return;}
 if(!['ArrowDown','ArrowUp','Home','End'].includes(event.key))return;
 event.preventDefault();event.stopPropagation();
 const items=[...event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)')];
 const index=items.indexOf(event.target as HTMLButtonElement);
 const next=event.key==='Home'?0:event.key==='End'?items.length-1:event.key==='ArrowDown'?(index+1)%items.length:(index-1+items.length)%items.length;
 items[next]?.focus();
}
