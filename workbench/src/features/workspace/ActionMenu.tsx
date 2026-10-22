import type {ReactNode} from "react";
import {Button} from "@essim/ui/components/button";
import {DropdownMenu,DropdownMenuContent,DropdownMenuItem,DropdownMenuTrigger} from "@essim/ui/components/dropdown-menu";
import {Tooltip,TooltipTrigger,TooltipContent} from "@essim/ui/components/tooltip";
export type ActionMenuItem = {label:string;disabled?:boolean;action:()=>void};
/** 低频外围操作集中到键盘可达的菜单，避免挤占点表。 */
export function ActionMenu({label,children,items,disabled=false}:{label:string;children?:ReactNode;disabled?:boolean;items:{label:string;disabled?:boolean;action:()=>void}[]}){
 const trigger=<DropdownMenuTrigger asChild><Button size="sm" variant="ghost" disabled={disabled} aria-label={label}>{children??label}</Button></DropdownMenuTrigger>;
 return <DropdownMenu>{children?<Tooltip><TooltipTrigger asChild><span className="toolbar-icon-wrap">{trigger}</span></TooltipTrigger><TooltipContent side="bottom" sideOffset={5}>{label}</TooltipContent></Tooltip>:trigger}<DropdownMenuContent align="end">{items.map(item=><DropdownMenuItem key={item.label} disabled={item.disabled} onSelect={item.action}>{item.label}</DropdownMenuItem>)}</DropdownMenuContent></DropdownMenu>;
}
