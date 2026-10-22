import type { ComponentProps } from "react";
import { Tooltip, TooltipTrigger, TooltipContent } from "@essim/ui/components/tooltip";
/** 图标操作始终提供名称；包裹层使禁用按钮也可显示原因。 */
export function ToolbarIcon({label,children,...props}:ComponentProps<"button"> & {label:string}) {
  return <Tooltip><TooltipTrigger asChild><span className="toolbar-icon-wrap"><button type="button" aria-label={label} {...props}>{children}</button></span></TooltipTrigger><TooltipContent side="bottom" sideOffset={5}>{label}</TooltipContent></Tooltip>;
}
