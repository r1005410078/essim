import {useRef,useState,type ReactNode} from "react";
import {Button} from "@essim/ui/components/button";
import {DropdownMenu,DropdownMenuContent,DropdownMenuItem,DropdownMenuTrigger} from "@essim/ui/components/dropdown-menu";
import {Tooltip,TooltipTrigger,TooltipContent} from "@essim/ui/components/tooltip";
export type ActionMenuItem = {label:string;icon?:ReactNode;disabled?:boolean;action:()=>void};
/** 低频外围操作集中到键盘可达的菜单，避免挤占点表。 */
export function ActionMenu({label,children,items,disabled=false}:{label:string;children?:ReactNode;disabled?:boolean;items:ActionMenuItem[]}){
 const [opened,setOpened]=useState(false);
 const triggerRef=useRef<HTMLButtonElement>(null);
 const trigger=<DropdownMenuTrigger asChild><Button ref={triggerRef} size="sm" variant="ghost" disabled={disabled} aria-label={label}>{children??label}</Button></DropdownMenuTrigger>;
 return <DropdownMenu modal={false} open={opened} onOpenChange={setOpened}>{children?<Tooltip><TooltipTrigger asChild><span className="toolbar-icon-wrap">{trigger}</span></TooltipTrigger><TooltipContent side="bottom" sideOffset={5}>{label}</TooltipContent></Tooltip>:trigger}{opened&&<DropdownMenuContent align="end" onCloseAutoFocus={event=>{if(document.querySelector("dialog[open]"))event.preventDefault();}}>{items.map(item=><DropdownMenuItem key={item.label} disabled={item.disabled} onSelect={()=>{triggerRef.current?.focus();item.action();}}>{item.icon}{item.label}</DropdownMenuItem>)}</DropdownMenuContent>}</DropdownMenu>;
}
