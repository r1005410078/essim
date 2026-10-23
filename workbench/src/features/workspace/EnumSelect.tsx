import { PointSelectItem } from "./PointSelectItem";
import { hasPointSelectOptions, pointSelectLabel } from "./pointSelectOptions";
import { useRef, useState } from "react";
import { Select, SelectContent, SelectTrigger, SelectValue } from "@essim/ui/components/select";
/** 选项按定义顺序显示，不将选中项前置。空值用内部占位映射。 */
export function EnumSelect({value,onChange,options,label,disabled=false,field}:{value:string;onChange:(value:string)=>void;options:readonly (readonly [string,string])[];label:string;disabled?:boolean;field?:string}) {
 const trigger=useRef<HTMLButtonElement>(null);
 const [container,setContainer]=useState<HTMLElement|undefined>();
 const empty="__essim_empty__";
 return <Select onOpenChange={open=>{if(open)setContainer(trigger.current?.closest("dialog") ?? undefined);}} value={value || empty} onValueChange={v=>onChange(v===empty?"":v)} disabled={disabled}>
  <SelectTrigger ref={trigger} aria-label={label} className="w-full min-w-0"><SelectValue placeholder="请选择">{field && hasPointSelectOptions(field) ? pointSelectLabel(field,value) || undefined : undefined}</SelectValue></SelectTrigger>
  <SelectContent portalContainer={container} position="popper" className={field && hasPointSelectOptions(field) ? "point-select-menu" : undefined}>{options.map(([id,name])=><PointSelectItem key={id || empty} field={field ?? ""} value={id || empty} textValue={name}>{name}</PointSelectItem>)}</SelectContent>
 </Select>;
}
