import {describe,it,expect} from "vitest";
import {resources,resourceNavigation,resourceSectionForMessage,markdownHeadings,resourcePanelId} from "./catalog";
import {documentPosition,projectNavigation} from "../workspace/model";
describe("内置文档资源",()=>{
 it("离线资源有稳定身份和分类，与采集模型平级",()=>{
  const tree=resourceNavigation();expect(tree.paths).toContain("文档资源/协议/Modbus/Modbus 详细指南");
  expect(tree.targets.get(tree.paths[0]!)).toEqual({kind:"resource",id:"modbus-guide"});
  const project=projectNavigation(undefined,"resources");expect(project.paths).toContain("文档资源/协议/Modbus/Modbus 详细指南");
 });
 it("功能码与异常都能定位正文中的真实章节",()=>{
  const guide=resources[0]!;const ids=new Set(markdownHeadings(guide.content).map(h=>h.id));
  for(const fc of [1,2,3,4,5,6,15,16])expect(ids.has(resourceSectionForMessage(fc,"RX"))).toBe(true);
  expect(ids.has(resourceSectionForMessage(0x83,"TX"))).toBe(true);
  expect(ids.has(resourceSectionForMessage(43,"RX"))).toBe(true);
 });
 it("锚点不改变Tab身份，欢迎页关闭后文档仍复用文档区",()=>{
  expect(resourcePanelId("modbus-guide")).toBe("resource:modbus-guide");
  expect(documentPosition(["project","resource:modbus-guide","observations"])).toEqual({referencePanel:"resource:modbus-guide",direction:"within"});
 });
 it("目录忽略代码块的伪标题，同名标题有唯一锚点",()=>{
  expect(markdownHeadings('## 地址\n```text\n## 假标题\n```\n## 地址').map(h=>h.id)).toEqual(["地址","地址-2"]);
 });
});

it("指南TCP教学报文可按请求／响应完整解析，不包含失配长度",async()=>{
 const {decodeModbusMessage}=await import("../workspace/modbusMessage");
 let request:string|undefined;let count=0;
 for(const line of resources[0]!.content.split("\n")){
  const match=line.match(/^(请求|响应)：([0-9A-F ]+)$/);if(!match)continue;
  const hex=match[2]!.trim(),direction=match[1]==="请求"?"RX":"TX";
  expect(decodeModbusMessage(hex,direction,request).issues,line).toEqual([]);
  if(direction==="RX")request=hex;count++;
 }
 expect(count).toBe(12);
});
