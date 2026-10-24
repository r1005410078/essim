import { createContext, useContext, useEffect, useId, useImperativeHandle, useLayoutEffect, useRef, useState, type ComponentProps, type CSSProperties, type ReactNode } from "react";
import { Search, X } from "lucide-react";
import { cn } from "../lib/utils";
import { propertyColumnRatio } from "../lib/property-columns";

const SearchContext = createContext("");
const ColumnContext = createContext<{
  ratio: number | null;
  resize: (pixels: number, width: number) => void;
  reset: () => void;
  setDragging: (dragging: boolean) => void;
} | null>(null);

export interface PropertyPanelProps extends Omit<ComponentProps<"section">, "title"> {
  title?: string;
  subject?: ReactNode;
  density?: "standard" | "compact";
  icon?: ReactNode;
  status?: ReactNode;
  actions?: ReactNode;
  filters?: ReactNode;
  search?: { value: string; onChange: (value: string) => void };
}

/** 通用属性面板外壳；只管理展示和按名称过滤，不读取业务状态。 */
export function PropertyPanel({ title = "详细信息", density = "standard", subject, icon, status, actions, filters, search, className, children, style, ref, ...props }: PropertyPanelProps) {
  const [ratio, setRatio] = useState<number | null>(null);
  const [effectiveRatio, setEffectiveRatio] = useState(.36);
  const [dragging, setDragging] = useState(false);
  const [guide,setGuide]=useState<{left:number;top:number;height:number}|null>(null);
  const panel = useRef<HTMLElement>(null);
  useImperativeHandle(ref, () => panel.current!, []);
  useLayoutEffect(() => {
    const element = panel.current;
    if (!element) return;
    const measure = () => {
      const rows = Array.from(element.querySelectorAll<HTMLElement>(".property-panel-row")).filter(row => row.getBoundingClientRect().width > 0);
      const row=rows[0];
      if (!row) {setGuide(null);return;}
      const bounds = row.getBoundingClientRect();
      const divider=row.children[1]!.getBoundingClientRect().left;
      setEffectiveRatio((divider - bounds.left) / bounds.width);
      if(dragging){
        const panelBounds=element.getBoundingClientRect();
        const next={left:divider-panelBounds.left+element.scrollLeft-element.clientLeft,top:bounds.top-panelBounds.top+element.scrollTop-element.clientTop,height:rows.at(-1)!.getBoundingClientRect().bottom-bounds.top};
        setGuide(previous=>previous&&previous.left===next.left&&previous.top===next.top&&previous.height===next.height?previous:next);
      }
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    element.addEventListener('toggle',measure,true);
    return () => {observer.disconnect();element.removeEventListener('toggle',measure,true);};
  }, [ratio, children, style,dragging]);
  return <section ref={panel} className={cn("property-panel", className)} aria-label={title} data-density={density} data-searching={!!search?.value.trim()} data-column-resizing={dragging}
    style={{...style, ...(ratio === null ? {} : {"--property-user-label-width": `${ratio * 100}%`})} as CSSProperties} {...props}>
    <header className="property-panel-header">
      {icon}<strong>{title}</strong>
      {subject && <span className="property-panel-subject" title={typeof subject === "string" ? subject : undefined}>{subject}</span>}
      <span className="property-panel-status">{status}</span>{actions}
    </header>
    {(search || filters) && <div className="property-panel-toolbar">
    {search && <div className="property-panel-search">
      <Search aria-hidden="true" size={13}/>
      <input type="search" aria-label="搜索属性" placeholder="搜索属性" value={search.value} onChange={event => search.onChange(event.target.value)}/>
      {search.value && <button type="button" aria-label="清除属性搜索" onClick={() => search.onChange("")}><X size={13}/></button>}
    </div>}
    {filters}
    </div>}
    <ColumnContext.Provider value={{ratio: effectiveRatio, resize: (pixels, width) => setRatio(propertyColumnRatio(pixels, width)), reset: () => setRatio(null), setDragging}}>
      <SearchContext.Provider value={search?.value.trim().toLocaleLowerCase() ?? ""}>{children}</SearchContext.Provider>
    </ColumnContext.Provider>
    {dragging && guide && <div aria-hidden="true" className="property-panel-column-guide" style={guide}/>}
    {search?.value.trim() && <div className="property-panel-search-empty" role="status">没有匹配的属性</div>}
  </section>;
}

export interface PropertyGroupProps extends Omit<ComponentProps<"details">, "title" | "open"> {
  title: string;
  defaultOpen?: boolean;
}

/** 使用原生 details 保留键盘展开与折叠，不承担配置应用行为。 */
export function PropertyGroup({ title, defaultOpen = true, className, children, onToggle, ...props }: PropertyGroupProps) {
  const search = useContext(SearchContext);
  const [expanded, setExpanded] = useState(defaultOpen);
  return <details className={cn("property-panel-group", className)} open={search ? true : expanded} onToggle={event => {
    if (!search) setExpanded(event.currentTarget.open);
    onToggle?.(event);
  }} {...props}><summary>{title}</summary>{children}</details>;
}

export interface PropertyRowProps extends Omit<ComponentProps<"div">, "children"> {
  label: string;
  help?: ReactNode;
  field?: string;
  keywords?: string;
  children: ReactNode;
}

/** 统一标签列和值列。控件由调用者提供，并显式设置自己的可访问名称。 */
export function PropertyRow({ label, help, field, keywords = "", className, children, ...props }: PropertyRowProps) {
  const search = useContext(SearchContext);
  const labelId = useId();
  if (search && !`${label} ${keywords}`.toLocaleLowerCase().includes(search)) return null;
  return <div className={cn("property-panel-row", className)} role="group" aria-labelledby={labelId} data-config-field={field} {...props}>
    <span id={labelId}>{label}{help}</span><div>{children}</div><PropertyColumnHandle/>
  </div>;
}

/** 每行提供同一列边界的命中区，列宽状态只由最近的面板持有。 */
function PropertyColumnHandle() {
  const columns = useContext(ColumnContext);
  const drag = useRef<{pointerId: number; x: number; labelWidth: number} | null>(null);
  const setDragging = columns?.setDragging;
  useEffect(() => () => {
    if (drag.current) setDragging?.(false);
  }, [setDragging]);
  if (!columns) return null;
  const geometry = (handle: HTMLButtonElement) => {
    const row = handle.parentElement!;
    const bounds = row.getBoundingClientRect();
    const value = row.children[1]!.getBoundingClientRect();
    return {width: bounds.width, labelWidth: value.left - bounds.left};
  };
  return <button className="property-panel-column-handle" type="button" role="separator" aria-label="调整字段名与值的列宽"
    aria-orientation="vertical" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round((columns.ratio ?? .36) * 100)}
    title="拖动调整整个面板列宽；方向键微调，双击或 Home 恢复默认"
    onPointerDown={event => {
      if (event.button !== 0) return;
      event.preventDefault(); event.stopPropagation();
      event.currentTarget.focus({preventScroll: true});
      const {labelWidth} = geometry(event.currentTarget);
      drag.current = {pointerId: event.pointerId, x: event.clientX, labelWidth};
      event.currentTarget.setPointerCapture(event.pointerId);
      columns.setDragging(true);
    }}
    onPointerMove={event => {
      const start = drag.current;
      if (!start || start.pointerId !== event.pointerId) return;
      const {width} = geometry(event.currentTarget);
      columns.resize(start.labelWidth + event.clientX - start.x, width);
    }}
    onPointerUp={event => {
      if (drag.current?.pointerId !== event.pointerId) return;
      drag.current = null; columns.setDragging(false);
      event.currentTarget.releasePointerCapture(event.pointerId);
    }}
    onPointerCancel={() => {drag.current = null; columns.setDragging(false);}}
    onLostPointerCapture={() => {drag.current = null; columns.setDragging(false);}}
    onDoubleClick={event => {event.preventDefault(); event.stopPropagation(); columns.reset();}}
    onKeyDown={event => {
      if (event.key === "Home") {event.preventDefault(); columns.reset(); return;}
      if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
      event.preventDefault(); event.stopPropagation();
      const {width, labelWidth} = geometry(event.currentTarget);
      columns.resize(labelWidth + (event.key === "ArrowLeft" ? -1 : 1) * (event.shiftKey ? 24 : 8), width);
    }}/>
}

/** 复合值容器：引用选择、定位、清除等操作共享值列，不压缩图标按钮。 */
export function PropertyActions({ className, ...props }: ComponentProps<"div">) {
  return <div className={cn("property-panel-actions", className)} {...props}/>;
}

export function PropertyIconButton({ label, className, ...props }: Omit<ComponentProps<"button">, "aria-label"> & { label: string }) {
  return <button type="button" aria-label={label} title={label} className={cn("property-panel-icon", className)} {...props}/>;
}

export function PropertyHint({ className, ...props }: ComponentProps<"div">) {
  return <div className={cn("property-panel-hint", className)} {...props}/>;
}

/** 范围过滤按钮组。调用者决定显示哪些分组，不伪装成独立文档 Tab。 */
export function PropertyScope<T extends string>({ value, onChange, options, label = "属性范围" }: {
  value: T;
  onChange: (value: T) => void;
  options: readonly { value: T; label: string }[];
  label?: string;
}) {
  return <div className="property-panel-scope" role="group" aria-label={label}>
    {options.map(option => <button type="button" key={option.value} aria-pressed={option.value === value} onClick={() => onChange(option.value)}>{option.label}</button>)}
  </div>;
}
