import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { PropertyPanel, PropertyGroup, PropertyRow, PropertyActions, PropertyIconButton } from "@essim/ui/components/property-panel";
import { propertyColumnRatio } from "@essim/ui/lib/property-columns";

it("名称列可缩至零且越界不产生负宽，值列继续保留可用宽度", () => {
  expect(propertyColumnRatio(10, 340) * 340).toBeCloseTo(10);
  expect(propertyColumnRatio(-80, 340)).toBe(0);
  expect(propertyColumnRatio(330, 340) * 340).toBeCloseTo(220);
  expect(propertyColumnRatio(150, 340) * 340).toBeCloseTo(150);
  expect(propertyColumnRatio(0, 200)).toBe(0);
  expect(propertyColumnRatio(1000, 200)).toBeCloseTo(.45);
  expect(propertyColumnRatio(100, 0)).toBe(.36);
});

it("搜索只筛字段名称和显式关键词，不泄露值或影响相邻面板", () => {
  const html = renderToStaticMarkup(<>
    <PropertyPanel search={{value:"  UNIT ",onChange:()=>{}}}>
      <PropertyGroup title="配置" defaultOpen={false}>
        <PropertyRow label="站号" keywords="unit id"><input aria-label="站号" defaultValue="7"/></PropertyRow>
        <PropertyRow label="说明">UNIT 只是字段值</PropertyRow>
      </PropertyGroup>
    </PropertyPanel>
    <PropertyPanel><PropertyRow label="另一面板">保留内容</PropertyRow></PropertyPanel>
  </>);
  expect(html).toContain('aria-label="站号"');
  expect(html).not.toContain("UNIT 只是字段值");
  expect(html).toContain("保留内容");
  expect(html).toContain('open=""');
});

it("复合字段有独立可访问名称，操作按钮不触发表单提交，错误定位标记保留", () => {
  const html = renderToStaticMarkup(<PropertyPanel><PropertyRow label="模型" field="model">
    <PropertyActions><input aria-label="模型名称" defaultValue="模型A"/><PropertyIconButton label="打开模型">↗</PropertyIconButton></PropertyActions>
  </PropertyRow></PropertyPanel>);
  expect(html).toContain('data-config-field="model"');
  expect(html).toMatch(/role="group" aria-labelledby="([^"]+)"[^>]*><span id="\1"/);
  expect(html).toContain('type="button" aria-label="打开模型"');
  expect(html).not.toContain("<label");
});
