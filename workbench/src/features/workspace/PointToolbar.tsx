import {createContext,useContext,useState,type ReactNode} from 'react';
import {createPortal} from 'react-dom';
import {Activity,SlidersHorizontal} from 'lucide-react';
import {Tooltip,TooltipTrigger,TooltipContent} from '@essim/ui/components/tooltip';
export type UnitViewMode='configuration'|'runtime';
const ToolbarHost=createContext<{target:HTMLDivElement|null;mode:UnitViewMode}|null>(null);
/** 端点只挂载一个工具栏；模式控制器声明式提供操作，不移动已有DOM。 */
export function EndpointToolbarFrame({mode,start,end,children}:{mode:UnitViewMode;start:ReactNode;end:ReactNode;children:ReactNode}){
 const [target,setTarget]=useState<HTMLDivElement|null>(null);
 return <ToolbarHost.Provider value={{target,mode}}><div className="panel column endpoint-panel">
  <div className="toolbar compact-point-toolbar endpoint-toolbar" role="toolbar" aria-label="单元工具栏">
   {start}<div className="endpoint-toolbar-actions" ref={setTarget}/>{end}
  </div>{children}
 </div></ToolbarHost.Provider>;
}
export function PointToolbar({mode='configuration',label='点表工具栏',className='',children}:{mode?:UnitViewMode;label?:string;className?:string;children:ReactNode}){
 const host=useContext(ToolbarHost);
 if(host)return host.mode===mode&&host.target?createPortal(<div className={`point-toolbar-actions-content ${className}`}>{children}</div>,host.target):null;
 return <div className={`toolbar compact-point-toolbar ${className}`} role="toolbar" aria-label={label}>{children}</div>;
}
/** 固定几何尺寸的单选组，切换时不卸载。 */
export function UnitViewSwitch({value,onChange}:{value:UnitViewMode;onChange:(value:UnitViewMode)=>void}){
 return <div className="unit-view-switch" role="tablist" aria-label="单元视图">{([['configuration','配置视图',SlidersHorizontal],['runtime','运行视图',Activity]] as const).map(([mode,label,Icon],index)=><Tooltip key={mode}><TooltipTrigger asChild><button type="button" role="tab" aria-label={label} aria-selected={value===mode} tabIndex={value===mode?0:-1} onClick={()=>onChange(mode)} onKeyDown={event=>{
  if(!['ArrowLeft','ArrowRight','Home','End'].includes(event.key))return;
  event.preventDefault();const next=event.key==='Home'?0:event.key==='End'?1:1-index;
  const buttons=event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>('[role="tab"]');buttons?.[next]?.focus();onChange(next===0?'configuration':'runtime');
 }}><Icon size={15}/></button></TooltipTrigger><TooltipContent>{label}</TooltipContent></Tooltip>)}</div>;
}
