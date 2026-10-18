import { useRef, useState } from "react";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@essim/ui/components/select";
/** 选项按定义顺序显示，不将选中项前置。空值用内部占位映射。 */
export function EnumSelect({value,onChange,options,label,disabled=false}:{value:string;onChange:(value:string)=>void;options:readonly (readonly [string,string])[];label:string;disabled?:boolean}) {
 const trigger=useRef<HTMLButtonElement>(null);
 const [container,setContainer]=useState<HTMLElement|undefined>();
 const empty="__essim_empty__";
 return <Select onOpenChange={open=>{if(open)setContainer(trigger.current?.closest("dialog") ?? undefined);}} value={value || empty} onValueChange={v=>onChange(v===empty?"":v)} disabled={disabled}>
  <SelectTrigger ref={trigger} aria-label={label} className="w-full min-w-0"><SelectValue placeholder="请选择" /></SelectTrigger>
  <SelectContent portalContainer={container} position="popper">{options.map(([id,name])=><SelectItem key={id || empty} value={id || empty}>{name}</SelectItem>)}</SelectContent>
 </Select>;
}
