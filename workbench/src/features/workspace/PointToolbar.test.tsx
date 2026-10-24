import {renderToStaticMarkup} from 'react-dom/server';
import {expect,it} from 'vitest';
import {TooltipProvider} from '@essim/ui/components/tooltip';
import {EndpointToolbarFrame,PointToolbar,UnitViewSwitch} from './PointToolbar';

it.each(['configuration','runtime'] as const)('在%s视图只提供一个工具栏和一组视图选择',mode=>{
 const html=renderToStaticMarkup(<TooltipProvider><EndpointToolbarFrame mode={mode} start={<button>当前单元</button>} end={<UnitViewSwitch value={mode} onChange={()=>{}}/>}>
  <PointToolbar><button>编辑操作</button></PointToolbar>
  <PointToolbar mode="runtime"><button>运行操作</button></PointToolbar>
 </EndpointToolbarFrame></TooltipProvider>);
 expect(html.match(/role="toolbar"/g)).toHaveLength(1);
 expect(html.match(/role="tablist"/g)).toHaveLength(1);
 expect(html.match(/当前单元/g)).toHaveLength(1);
 expect(html).toMatch(new RegExp(`role="tab" aria-label="${mode==='configuration'?'配置视图':'运行视图'}" aria-selected="true" tabindex="0"`));
 expect(html.match(/aria-selected="false" tabindex="-1"/g)).toHaveLength(1);
});

it('独立模型编辑器仍提供自己的可访问工具栏',()=>{
 const html=renderToStaticMarkup(<PointToolbar><button>保存模型</button></PointToolbar>);
 expect(html).toContain('role="toolbar"');
 expect(html).toContain('保存模型');
});
