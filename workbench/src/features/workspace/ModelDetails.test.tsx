import {renderToStaticMarkup} from 'react-dom/server';
import {expect,it} from 'vitest';
import {TooltipProvider} from '@essim/ui/components/tooltip';
import {ModelDetails} from './PointEditor';
import {WorkspaceStore} from './model';
import {ModelDraft,defaultRow,gridFields,labels} from './pointDraft';
import type {WorkspaceClient} from '../../shared/api/contracts';

it('紧凑分组保留全部点表字段及列宽手柄，预览默认折叠且不重新出现分类和备注',()=>{
 const store=new WorkspaceStore({} as WorkspaceClient);
 const row={...defaultRow('measurement'),id:'point-1',name:'测试电压'};
 const draft=new ModelDraft('model-1','测试模型',[row],()=>{});
 draft.selected='point-1';store.modelDrafts.set(draft.id,draft);
 const html=renderToStaticMarkup(<TooltipProvider><ModelDetails store={store} id={draft.id} open={()=>{}}/></TooltipProvider>);
 expect(html).toContain('data-density="compact"');
 for(const field of gridFields)expect(html).toContain(labels[field]);
 for(const title of ['点定义','数值解释','访问与采集'])expect(html).toContain(`<summary>${title}</summary>`);
 expect(html).toMatch(/<details[^>]*><summary>编码预览<\/summary>/);
 expect(html).not.toContain('四遥分类');expect(html).not.toContain('>备注<');
 expect(html).toContain('aria-label="调整字段名与值的列宽"');
 expect(draft.rows[0]?.name).toBe('测试电压');
});

it('模型上下文切换范围不重复显示点字段，没有关联对象时明确提示',()=>{
 const store=new WorkspaceStore({} as WorkspaceClient);
 const draft=new ModelDraft('model-scope','模型范围测试',[{...defaultRow('measurement'),id:'point-1'}],()=>{});
 draft.selected='point-1';store.modelDrafts.set(draft.id,draft);
 const render=(scope:'unit'|'endpoint')=>renderToStaticMarkup(<TooltipProvider><ModelDetails store={store} id={draft.id} scope={scope} open={()=>{}}/></TooltipProvider>);
 const unit=render('unit'),endpoint=render('endpoint');
 expect(unit).toContain('关联 Unit');expect(unit).toContain('当前模型没有关联 Unit');
 expect(endpoint).toContain('关联端点');expect(endpoint).toContain('当前模型没有关联端点');
 for(const html of [unit,endpoint]){expect(html).not.toContain('<summary>点定义</summary>');expect(html).not.toContain('aria-label="零基地址"');}
});
