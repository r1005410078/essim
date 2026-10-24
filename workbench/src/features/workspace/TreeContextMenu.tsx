import {useLayoutEffect,useRef,type HTMLAttributes} from 'react';
import {menuKeyboard} from './menuKeyboard';
/** 树库提供定位及关闭；自定义内容负责菜单内部焦点与键盘导航。 */
export function TreeContextMenu({close,...props}:HTMLAttributes<HTMLDivElement>&{close:()=>void}){
 const ref=useRef<HTMLDivElement>(null);
 useLayoutEffect(()=>{let live=true;queueMicrotask(()=>{if(live)ref.current?.querySelector<HTMLButtonElement>('[role="menuitem"]:not(:disabled)')?.focus();});return()=>{live=false;};},[]);
 return <div {...props} ref={ref} role="menu" onKeyDown={event=>menuKeyboard(event,close)}/>;
}
