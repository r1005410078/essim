import {useMemo,useRef,useState} from "react";
import {Popover,PopoverContent,PopoverTrigger,PopoverClose} from "@essim/ui/components/popover";
import {Button} from "@essim/ui/components/button";
import {X,BookOpen} from "lucide-react";
import {resourceSectionForMessage} from "../resources/catalog";
import {decodeRtuMessage} from "./modbusRtuMessage";
import {decodeModbusMessage} from "./modbusMessage";

/** 浮层只解释记录中的真实字节，点击或Enter打开，浮层内支持详细文档入口。 */
export function MessagePopover({hex,direction,requestHex,compact=false,master=false,transport="tcp",onDetails}:{hex:string;direction:"RX"|"TX";requestHex?:string;compact?:boolean;master?:boolean;transport?:"tcp"|"rtu";onDetails:(anchor:string)=>void}){
 const [open,setOpen]=useState(false);const navigating=useRef(false);
 const protocolDirection=master?(direction==="RX"?"TX":"RX"):direction;
 const decoded=useMemo(()=>open?(transport==="rtu"?decodeRtuMessage:decodeModbusMessage)(hex,protocolDirection,requestHex):null,[open,hex,protocolDirection,requestHex,transport]);
 return <Popover open={open} onOpenChange={setOpen}>
  <PopoverTrigger asChild><button type="button" className={`message-bytes ${compact?"message-bytes-inline":""}`} aria-label={`${direction} 报文，点击查看字段解析`}>{hex}</button></PopoverTrigger>
  <PopoverContent side="top" align="start" sideOffset={9} collisionPadding={12} className="modbus-popover" onCloseAutoFocus={event=>{if(navigating.current){event.preventDefault();navigating.current=false;}}} aria-label="Modbus 报文解析"><div className="modbus-tooltip">
   {decoded&&<>
    <div className="message-decode-title"><strong>{decoded.title}</strong><span className={`message-direction direction-${direction}`}>{direction}</span><span className="spacer"/><span>{hex.trim().split(/\s+/).length} 字节</span><PopoverClose asChild><Button variant="ghost" size="icon-sm" aria-label="关闭报文解析"><X size={14}/></Button></PopoverClose></div>
    <div className="message-decode-subtitle">{decoded.subtitle}</div>
    <div className="message-byte-groups">{decoded.fields.map(field=><code className={`byte-tone-${field.tone}`} key={field.offset}>{field.hex}</code>)}</div>
    {decoded.issues.length>0&&<div className="message-decode-errors" role="alert">{decoded.issues.map(issue=><div key={issue}>{issue}</div>)}</div>}
    <div className="message-field-list">{(transport==="rtu"?["RTU","PDU"] as const:["MBAP","PDU"] as const).map(section=><div className="message-field-section" key={section}><div className="message-section-label">{section}<small>{section==="MBAP"?"7 字节":section==="RTU"?"站号与CRC":"功能与数据"}</small></div><div>{decoded.fields.filter(field=>field.section===section).map(field=><div className="message-field" key={field.offset}><code className={`byte-tone-${field.tone}`}>{field.hex}</code><span>{field.description}</span></div>)}</div></div>)}</div>
    {decoded.values.length>0&&<div className="message-values">{decoded.values.map(value=><div key={value.label}><span>{value.label}</span><code className={value.value==="true"?"value-true":value.value==="false"?"value-false":""}>{value.value}</code></div>)}{decoded.valueCount>decoded.values.length&&<span className="message-more">共 {decoded.valueCount} 项，预览前 {decoded.values.length} 项</span>}</div>}
    {decoded.note&&<div className="message-decode-note">{decoded.note}</div>}
    <div className="message-decode-foot"><span>仅解析报文字段 · Escape 关闭</span><Button size="sm" variant="ghost" onClick={()=>{navigating.current=true;setOpen(false);const code=Number.parseInt(hex.trim().split(/\s+/)[transport==="rtu"?1:7]??"",16);onDetails(resourceSectionForMessage(Number.isNaN(code)?undefined:code,protocolDirection));}}><BookOpen size={14}/>详细</Button></div>
   </>}
  </div></PopoverContent>
 </Popover>;
}
