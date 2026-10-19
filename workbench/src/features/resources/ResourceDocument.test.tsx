import {it,expect} from "vitest";
import {renderToStaticMarkup} from "react-dom/server";
import {ResourceDocument} from "./ResourceDocument";
it("真实资源正文由Markdown渲染出章节、表格和代码，注册锚点可定位",()=>{
 const html=renderToStaticMarkup(<ResourceDocument id="modbus-guide"/>);
 expect(html).toContain('id="fc15-写多个线圈"');
 expect(html).toContain('<table>');expect(html).toContain('<pre>');
 expect(html).toContain('aria-label="文档章节目录"');
 expect(html).toContain('target="_blank" rel="noopener noreferrer"');
 expect(html).not.toContain('## FC15');
});
it("不存在的资源显示明确空态",()=>{
 expect(renderToStaticMarkup(<ResourceDocument id="missing"/>)).toContain("文档资源不存在");
});
