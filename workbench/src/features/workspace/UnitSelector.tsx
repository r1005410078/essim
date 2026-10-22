import { useEffect, useId, useRef, useState } from "react";
import { Check, ChevronDown, Search } from "lucide-react";
import { Popover, PopoverTrigger, PopoverContent } from "@essim/ui/components/popover";
import { describeUnit, filterUnits, type SelectableUnit } from "./unitOptions";

/** 只切换当前视图，禁用单元也可查看；始终用稳定ID通知业务层。 */
export function UnitSelector({units,value,onChange,disabled=false}: {
  units: readonly SelectableUnit[]; value: string; onChange: (id:string)=>void; disabled?:boolean;
}) {
  const [open,setOpen] = useState(false);
  const [query,setQuery] = useState("");
  const [active,setActive] = useState(0);
  const list = useRef<HTMLDivElement>(null);
  const id = useId();
  const selected = units.find(unit=>unit.id===value);
  const found = filterUnits(units,query);
  useEffect(()=>{list.current?.querySelector<HTMLElement>(`[data-active="true"]`)?.scrollIntoView({block:"nearest"});},[active,query,open]);
  const content = (unit:SelectableUnit) => {
    const display=describeUnit(unit);
    return <>{display.name ? <><span className="unit-id-badge">{display.number}</span><span className="unit-selector-name">{display.name}</span></> : <span className="unit-selector-name">{display.label}</span>}</>;
  };
  const choose = (index:number) => {const unit=found[index];if(!unit)return;onChange(unit.id);setOpen(false);};
  return <Popover open={open} onOpenChange={next=>{setOpen(next);setQuery("");setActive(Math.max(0,units.findIndex(unit=>unit.id===value)));}}>
    <PopoverTrigger asChild><button type="button" className="unit-selector-trigger" disabled={disabled||!units.length} aria-label="切换单元" title={selected?`${describeUnit(selected).label}${selected.enabled?"":" · 已禁用"}`:"未添加单元"}>
      {selected?content(selected):<span>未添加单元</span>}<ChevronDown size={12}/>
    </button></PopoverTrigger>
    <PopoverContent align="start" sideOffset={4} className="unit-selector-popup">
      <div className="unit-selector-search"><Search size={13}/><input aria-label="搜索名称或 Unit ID" role="combobox" aria-expanded="true" aria-controls={id} aria-autocomplete="list" aria-activedescendant={found[active]?`${id}-${active}`:undefined} placeholder="搜索名称 / Unit ID" value={query} onChange={e=>{setQuery(e.target.value);setActive(0);}} onKeyDown={e=>{
        if(e.nativeEvent.isComposing)return;
        if(e.key==="ArrowDown"||e.key==="ArrowUp"){e.preventDefault();setActive(i=>Math.max(0,Math.min(found.length-1,i+(e.key==="ArrowDown"?1:-1))));}
        if(e.key==="Enter"){e.preventDefault();choose(active);}
      }}/></div>
      <div ref={list} id={id} role="listbox" aria-label="单元列表" className="unit-selector-list">
        {found.map((unit,index)=><button type="button" role="option" id={`${id}-${index}`} key={unit.id} aria-selected={unit.id===value} data-active={index===active} onPointerMove={()=>setActive(index)} onMouseDown={e=>e.preventDefault()} onClick={()=>choose(index)} title={describeUnit(unit).label}>
          {content(unit)}{!unit.enabled&&<small>已禁用</small>}{unit.id===value&&<Check size={13} className="unit-selector-check"/>}
        </button>)}
        {!found.length&&<div className="unit-selector-empty">没有匹配的单元</div>}
      </div>
      <div className="unit-selector-footer"><span>{found.length} / {units.length} 个单元</span><span>↑ ↓ 选择　↵ 切换</span></div>
    </PopoverContent>
  </Popover>;
}
