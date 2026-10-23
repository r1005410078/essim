import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import { TooltipProvider } from "@essim/ui/components/tooltip";
import { ConfigurationStatus, RuntimeError } from "./RuntimeToolbar";

function status(props: Parameters<typeof ConfigurationStatus>[0]) {
  return renderToStaticMarkup(<TooltipProvider><ConfigurationStatus {...props}/></TooltipProvider>);
}

it("无快照时说明首次应用，已有快照的待应用状态仍说明使用旧快照", () => {
  const first = status({dirty:false,applied:false,hasSnapshot:false,master:false});
  expect(first).toContain("首次运行前，请先保存并应用配置");
  expect(first).toContain(">未应用</span>");
  const pending = status({dirty:false,applied:false,hasSnapshot:true,master:true});
  expect(pending).toContain("连接和采集使用已应用快照");
  expect(pending).toContain(">待应用</span>");
  expect(pending).not.toContain("首次运行前");
  expect(pending).not.toContain('role="alert"');
});

it("保存与应用状态分开，模型未保存提醒保留在可访问提示中", () => {
  const html = status({dirty:true,applied:true,hasSnapshot:true,master:false,modelDirty:true});
  expect(html).toContain(">未保存</span>");
  expect(html).toContain("引用模型有未保存修改");
  expect(html).toContain("启动使用已应用快照");
  const applied = status({dirty:false,applied:true,hasSnapshot:true,master:false});
  expect(applied).toContain(">已应用</span>");
  expect(applied).not.toContain("pending");
});

it("真实错误及独立的未确认结果直接显示为警报，正常状态不占提示行", () => {
  expect(renderToStaticMarkup(<RuntimeError/>)).toBe("");
  const error = renderToStaticMarkup(<RuntimeError error="打开串口失败：设备忙" uncertain/>);
  expect(error).toContain('role="alert"');
  expect(error).toContain("打开串口失败：设备忙；存在未确认结果，请勿据此重复写入。");
  expect(renderToStaticMarkup(<RuntimeError uncertain/>)).toContain('role="alert"');
  expect(renderToStaticMarkup(<RuntimeError error="草稿不合法" runtimeError="串口连接断开"/>)).toContain("草稿不合法；串口连接断开");
  const repeated = renderToStaticMarkup(<RuntimeError error="请求超时" runtimeError="请求超时"/>);
  expect(repeated.split("请求超时")).toHaveLength(2);
});
