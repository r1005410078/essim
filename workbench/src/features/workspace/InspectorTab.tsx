import {Layers,ListTree,Server} from 'lucide-react';
import {useEffect,useRef,useState} from 'react';
import {Tooltip,TooltipTrigger,TooltipContent} from '@essim/ui/components/tooltip';
import type {IDockviewPanelHeaderProps} from 'dockview-react';
import {inspectorTabs} from './inspectorTabs';
/** 内容仅图标；原生Dockview外层负责选择、焦点和标签语义。 */
export function InspectorTab({api}:IDockviewPanelHeaderProps){
 const trigger=useRef<HTMLSpanElement>(null);
 const [open,setOpen]=useState(false);
 const tab=inspectorTabs.find(tab=>tab.id===api.id)!;
 const Icon=tab.scope==='point'?ListTree:tab.scope==='unit'?Layers:Server;
 // 键盘焦点由Dockview外层tab持有，不另加嵌套的可聚焦控件。
 useEffect(()=>{
   const element=trigger.current?.closest('[role="tab"]');
   const focus=()=>setOpen(true),blur=()=>setOpen(false);
   element?.addEventListener('focusin',focus);element?.addEventListener('focusout',blur);
   return()=>{element?.removeEventListener('focusin',focus);element?.removeEventListener('focusout',blur);};
 },[]);
 return <Tooltip open={open} onOpenChange={setOpen}><TooltipTrigger asChild><span ref={trigger} className="inspector-edge-tab" aria-label={tab.title}><Icon size={17} aria-hidden="true"/></span></TooltipTrigger><TooltipContent side="left" sideOffset={6}>{tab.title}属性</TooltipContent></Tooltip>;
}
