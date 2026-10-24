import {useState} from 'react';
import {Search} from 'lucide-react';
import {folderIcons,folderColors,folderIcon,FolderGlyph} from './folderIcons';
const groups=[['协议与项目','协议'],['储能设备','储能'],['系统与测试','系统']] as const;
/** 图标与颜色选择保持在同一紧凑区域，名称通过悬停与可访问标签提供。 */
export function FolderAppearance({icon,color,onIcon,onColor,disabled}:{icon:string;color:string;onIcon:(value:string)=>void;onColor:(value:string)=>void;disabled:boolean}){
 const [query,setQuery]=useState('');const [group,setGroup]=useState(folderIcon(icon).group);
 const selected=folderIcon(icon);
 const options=folderIcons.filter(item=>query?`${item.label} ${item.id}`.toLowerCase().includes(query.trim().toLowerCase()):item.group===group);
 return <fieldset className="folder-appearance" disabled={disabled} aria-label="文件夹外观">
  <div className="folder-icon-picker">
   <div className="folder-icon-toolbar">
    <div className="folder-icon-groups" role="group" aria-label="图标分类">{groups.map(([value,label])=><button key={value} type="button" aria-label={value} aria-pressed={value===group} onClick={()=>{setGroup(value);setQuery('');}}>{label}</button>)}</div>
    <label className="folder-icon-search"><Search size={13} aria-hidden="true"/><input aria-label="查找文件夹图标" value={query} onChange={e=>setQuery(e.target.value)} placeholder="搜索图标"/></label>
   </div>
   <div className="folder-icon-grid" role="group" aria-label="主题图标">{options.map(item=><button key={item.id} type="button" title={item.label} aria-label={item.label} aria-pressed={icon===item.id} onClick={()=>onIcon(item.id)} onKeyDown={event=>{
    if(!['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(event.key))return;
    event.preventDefault();const offset=event.key==='ArrowLeft'?-1:event.key==='ArrowRight'?1:event.key==='ArrowUp'?-10:10;
    const index=options.findIndex(i=>i.id===item.id);const next=options[(index+offset+options.length)%options.length];
    if(next){onIcon(next.id);event.currentTarget.parentElement?.querySelector<HTMLButtonElement>(`[aria-label="${next.label}"]`)?.focus();}
   }}><FolderGlyph icon={item.id} color={color} size={18}/></button>)}</div>
   {!options.length&&<p className="folder-icon-empty" role="status">没有匹配的图标</p>}
  </div>
  <div className="folder-color-row" role="group" aria-label="图标颜色">
   <span>颜色</span>
   <div className="folder-color-swatches">{folderColors.map((value,index)=><button key={value} type="button" title={['蓝色','青绿色','琥珀色','紫色','灰色'][index]} aria-label={['蓝色','青绿色','琥珀色','紫色','灰色'][index]} aria-pressed={value.toLowerCase()===color.toLowerCase()} style={{color:value}} onClick={()=>onColor(value)}><span style={{background:value}}/></button>)}</div>
   <input aria-label="自定义图标颜色" title="自定义颜色" type="color" value={/^#[0-9a-f]{6}$/i.test(color)?color:'#477fbc'} onChange={e=>onColor(e.target.value)}/>
   <input aria-label="颜色十六进制" required pattern="#[0-9a-fA-F]{6}" value={color} onChange={e=>onColor(e.target.value)} maxLength={7}/>
   <span className="folder-selected-name" title={selected.label}>{selected.label}</span>
  </div>
 </fieldset>;
}
