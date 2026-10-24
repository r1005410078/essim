import type {ReactNode} from 'react';
import {ChevronLeft,ChevronRight} from 'lucide-react';
import type {Category} from '../../shared/api/contracts';
import {categories} from './sheets';

/** 编辑与运行点表共用四遥底栏，切换前的草稿处理由调用方负责。 */
export function CategoryTabs({value,onChange,label='四遥分类',hidden=false,children}:{
 value:Category;onChange:(value:Category)=>void;label?:string;hidden?:boolean;children?:ReactNode;
}){
 const step=(offset:number)=>onChange(categories[(categories.findIndex(([category])=>category===value)+offset+categories.length)%categories.length]![0]);
 return <div className="category-tabs" role="tablist" aria-label={label} style={{display:hidden?'none':undefined}}>
  <div className="category-tab-navigation" role="group" aria-label="分类导航">
  <button type="button" aria-label="上一分类" onClick={()=>step(-1)}><ChevronLeft size={13}/></button>
  <button type="button" aria-label="下一分类" onClick={()=>step(1)}><ChevronRight size={13}/></button>
  </div>
  {categories.map(([category,title])=><button type="button" key={category} role="tab" aria-selected={value===category} className={value===category?'active':''} onClick={()=>onChange(category)}>{title}</button>)}
  <span className="spacer"/><span className="muted">{children}</span>
 </div>;
}
