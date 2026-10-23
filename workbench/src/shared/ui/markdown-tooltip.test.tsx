import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { MarkdownTooltipBody } from "@essim/ui/components/markdown-tooltip";

it("公共说明渲染GFM和来源，正文中的HTML不执行", () => {
  const html = renderToStaticMarkup(<MarkdownTooltipBody markdown={'### 线圈\n\n**1 位**，`Coils`\n\n|功能|码|\n|---|---|\n|读|01|\n\n<script>alert(1)</script>'} source="Modbus §4.3"/>);
  expect(html).toContain("<h3>线圈</h3>");
  expect(html).toContain("<strong>1 位</strong>");
  expect(html).toContain("<code>Coils</code>");
  expect(html).toContain("<table>");
  expect(html).toContain("Modbus §4.3");
  expect(html).not.toContain("<script>");
});

it("没有来源时不显示空来源栏", () => {
  const html = renderToStaticMarkup(<MarkdownTooltipBody markdown="说明"/>);
  expect(html).toContain("<p>说明</p>");
  expect(html).not.toContain("markdown-tooltip-source");
});
