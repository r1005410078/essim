import { useState } from "react";
import { createRoot } from "react-dom/client";
import { FolderOpen, Plus, X } from "lucide-react";
import { PropertyPanel, PropertyGroup, PropertyRow, PropertyActions, PropertyIconButton, PropertyHint, PropertyScope } from "@essim/ui/components/property-panel";
import { EnumSelect } from "../features/workspace/EnumSelect";
import "@essim/ui/styles.css";
import "./property-panel-preview.css";

/** 独立开发样例：所有值为样式演示，不连接协议后端，也不进入正式应用路由。 */
function PreviewPanel({ width, serial = false }: { width: number; serial?: boolean }) {
  const [search, setSearch] = useState("");
  const [scope, setScope] = useState("unit");
  const [model, setModel] = useState("bms");
  const [message, setMessage] = useState("");
  return <article style={{width}}>
    <div className="preview-caption">{width}px · {serial ? "复合字段 / 长内容" : "标准密度 / 可编辑"}</div>
    <div className="preview-frame">
      <PropertyPanel subject={serial ? "RTU 从站 / Unit 1" : "TCP 从站 / Unit 1"} status="示例草稿" search={{value:search,onChange:setSearch}}
        filters={<PropertyScope value={scope} onChange={setScope} options={[{value:"unit",label:"Unit"},{value:"point",label:"点"}]}/>}>
        <PropertyGroup title="单元配置" hidden={!search && scope!=="unit"}>
          <PropertyRow label="当前单元"><PropertyActions><EnumSelect label="选择单元" value="unit" onChange={()=>{}} options={[["unit","储能单元 · Unit 1"]]}/><PropertyIconButton label="添加 Unit" onClick={()=>setMessage("示例操作：添加 Unit")}><Plus/></PropertyIconButton></PropertyActions></PropertyRow>
          <PropertyRow label="Unit ID" keywords="站号"><input aria-label="Unit ID" type="number" min={1} max={247} defaultValue="1"/></PropertyRow>
          <PropertyRow label="名称"><input aria-label="名称" defaultValue="储能单元"/></PropertyRow>
          <PropertyRow label="备注"><input aria-label="备注" placeholder="添加说明"/></PropertyRow>
          <PropertyRow label="启用"><input aria-label="启用" type="checkbox" defaultChecked/></PropertyRow>
          <PropertyRow label="关联设备"><PropertyActions><EnumSelect label="关联设备" value="none" onChange={()=>{}} options={[["none","独立模拟器"]]}/><PropertyIconButton label="定位设备" disabled><FolderOpen/></PropertyIconButton><PropertyIconButton label="解除关联" disabled><X/></PropertyIconButton></PropertyActions></PropertyRow>
          <PropertyRow label="采集模型"><PropertyActions><EnumSelect label="采集模型" value={model} onChange={setModel} options={[["bms","BMS 电池簇标准采集模型"],["pcs","PCS 变流器模型"]]}/><PropertyIconButton label="打开模型" onClick={()=>setMessage("示例操作：打开采集模型")}><FolderOpen/></PropertyIconButton></PropertyActions></PropertyRow>
        </PropertyGroup>
        <PropertyGroup title="通信参数" hidden={!search && scope!=="unit"}>
          <PropertyRow label="协议">Modbus {serial ? "RTU" : "TCP"}</PropertyRow>
          {serial ? <>
            <PropertyRow label="串口路径"><div style={{display:"grid",gap:3}}><EnumSelect label="已发现串口" value="manual" onChange={()=>{}} options={[["manual","手动路径"]]}/><input aria-label="串口路径" defaultValue="/dev/cu.usbserial-example"/><small>示例路径；未打开串口。</small></div></PropertyRow>
            <PropertyRow label="波特率"><EnumSelect label="波特率" value="19200" onChange={()=>{}} options={[["19200","19200"]]}/></PropertyRow>
          </> : <>
            <PropertyRow label="监听地址"><input aria-label="监听地址" defaultValue="127.0.0.1"/></PropertyRow>
            <PropertyRow label="端口"><input aria-label="端口" type="number" defaultValue="1502"/></PropertyRow>
          </>}
          <PropertyRow label="状态">未运行 · 仅样式预览</PropertyRow>
        </PropertyGroup>
        <PropertyGroup title="选中点" hidden={!search && scope!=="point"}>
          <PropertyRow label="点名"><input aria-label="点名" defaultValue="电池簇电压"/></PropertyRow>
          <PropertyRow label="零基地址"><input aria-label="零基地址" defaultValue="0"/></PropertyRow>
          <PropertyRow label="字序"><EnumSelect label="字序" value="big" disabled onChange={()=>{}} options={[["big","big"]]}/></PropertyRow>
          <PropertyRow label="允许外部写入"><input aria-label="允许外部写入" type="checkbox"/></PropertyRow>
        </PropertyGroup>
        <PropertyGroup title="扩展信息" defaultOpen={false}>
          <PropertyRow label="说明">新增协议字段按相同组件规范接入。</PropertyRow>
        </PropertyGroup>
        <PropertyHint>保存与应用保持独立；此页仅展示组件。</PropertyHint>
        {message && <PropertyHint role="status">{message}</PropertyHint>}
      </PropertyPanel>
    </div>
  </article>;
}

function Preview() {
  return <main className="property-preview">
    <header><span>ESSim / UI COMPONENTS</span><h1>紧凑属性面板</h1><p>连续属性行 · 分类折叠 · 标签与值对齐 · 浅色工程工作台</p></header>
    <div className="preview-panels"><PreviewPanel width={340}/><PreviewPanel width={280} serial/></div>
    <p className="preview-footnote">独立组件样例，非真实设备状态。可拖动分隔线同步调整列宽，双击恢复；也可尝试搜索、折叠和键盘操作。</p>
  </main>;
}
createRoot(document.getElementById("root")!).render(<Preview/>);
