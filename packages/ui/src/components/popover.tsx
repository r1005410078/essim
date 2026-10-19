import * as React from "react";
import {Popover as PopoverPrimitive} from "radix-ui";
import {cn} from "../lib/utils";
const Popover=PopoverPrimitive.Root;
const PopoverTrigger=PopoverPrimitive.Trigger;
const PopoverClose=PopoverPrimitive.Close;
/** 使用原生三角Arrow；箭头独立于可滚动内容，避免方块旋转和裁切。 */
function PopoverContent({className,children,sideOffset=9,...props}:React.ComponentProps<typeof PopoverPrimitive.Content>){
 return <PopoverPrimitive.Portal><PopoverPrimitive.Content data-slot="popover-content" sideOffset={sideOffset} className={cn("z-50 rounded-lg border border-border bg-popover text-popover-foreground shadow-md outline-none",className)} {...props}>
  {children}<PopoverPrimitive.Arrow width={14} height={7} className="popover-triangle"/>
 </PopoverPrimitive.Content></PopoverPrimitive.Portal>;
}
export {Popover,PopoverTrigger,PopoverClose,PopoverContent};
