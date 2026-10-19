import type {ReactNode} from "react";
import {Button} from "@essim/ui/components/button";
import {DropdownMenu,DropdownMenuContent,DropdownMenuItem,DropdownMenuTrigger} from "@essim/ui/components/dropdown-menu";
/** 低频外围操作集中到键盘可达的菜单，避免挤占点表。 */
export function ActionMenu({label,children,items,disabled=false}:{label:string;children?:ReactNode;disabled?:boolean;items:{label:string;disabled?:boolean;action:()=>void}[]}){
 return <DropdownMenu><DropdownMenuTrigger asChild><Button size="sm" variant="ghost" disabled={disabled} aria-label={label}>{children??label}</Button></DropdownMenuTrigger><DropdownMenuContent align="end">{items.map(item=><DropdownMenuItem key={item.label} disabled={item.disabled} onSelect={item.action}>{item.label}</DropdownMenuItem>)}</DropdownMenuContent></DropdownMenu>;
}
