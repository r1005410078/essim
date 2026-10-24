import { renderToStaticMarkup } from 'react-dom/server';
import { expect, it } from 'vitest';
import { WorkbenchDialog, DialogBody, DialogFooter } from '@essim/ui/components/workbench-dialog';
it('原生 dialog 有标题关联，头部工具不会误提交表单，忙碌时关闭禁用', () => {
  const html=renderToStaticMarkup(<WorkbenchDialog title="应用配置" busy onCancel={()=>{}}><form><DialogBody><input required aria-label="名称"/></DialogBody><DialogFooter><button type="button">取消</button><button type="submit">应用</button></DialogFooter></form></WorkbenchDialog>);
  expect(html).toMatch(/<dialog[^>]*aria-labelledby="([^"]+)"[^>]*aria-busy="true"/);
  expect(html).toMatch(/<h2 id="[^"]+">应用配置<\/h2>/);
  expect(html).toMatch(/type="button" aria-label="关闭弹框"[^>]*disabled=""/);
  expect(html).toContain('type="submit">应用');
  expect(html.match(/data-edge=/g)).toHaveLength(8);
  expect(html).toContain('aria-label="调整弹框大小（方向键调整，Home 复位）"');
});

it('命令变体复用相同dialog与表单结构，不影响默认弹框',()=>{
 const html=renderToStaticMarkup(<WorkbenchDialog variant="command" title="检索" aria-label="检索" onCancel={()=>{}}><form><DialogBody><input autoFocus/></DialogBody><DialogFooter><button type="submit">执行</button></DialogFooter></form></WorkbenchDialog>);
 expect(html).toContain('workbench-dialog-command');expect(html).toContain('aria-label="检索"');expect(html).toContain('type="submit">执行');
});
