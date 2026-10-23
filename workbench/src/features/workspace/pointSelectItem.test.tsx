import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { EnumSelect } from "./EnumSelect";
import { choices } from "./pointDraft";

it("属性栏类型、字节序与字序显示原标识，普通枚举保留原有标签", () => {
  for (const [field, value, name] of [["valueType","f64","f64"],["byteOrder","little","little"],["wordOrder","big","big"]]) {
    const html = renderToStaticMarkup(<EnumSelect field={field} label={field!} value={value!} onChange={()=>{}} options={choices[field!]!.map(id=>[id,id])}/>);
    expect(html).toContain(`>${name}</span>`);
    if (value !== name) expect(html).not.toContain(`>${value}</span>`);
    expect(html).not.toContain("point-select-option-meta");
  }
  const html = renderToStaticMarkup(<EnumSelect label="普通枚举" value="true" onChange={()=>{}} options={[["true","允许"]]}/>);
  expect(html).not.toContain("point-select-option-meta");
});
