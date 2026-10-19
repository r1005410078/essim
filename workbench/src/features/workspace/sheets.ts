import { RuntimeDataSource } from "./RuntimeDataSource";
/** 复用已有NovaSheet数据源；草稿按稳定单元身份和四遥分类隔离。 */
import {
  denseGridTheme,
} from "@zhiguang/novasheet-core";
import type {
  Catalog,
  EndpointConfig,
  UnitConfig,
  Category,
  CollectionModel,
  UnitValues,
  Role,
  MasterPoint,
} from "../../shared/api/contracts";
export const readonlyTypes = {
  readonly: {
    editable: false,
    serializeClipboard: (value: unknown) =>
      value == null ? "" : String(value),
  },
};
export const sheetTheme = {
  ...denseGridTheme,
  metrics: {
    ...denseGridTheme.metrics,
    rowHeaderWidth: 38,
    rowHeight: 28,
    headerHeight: 31,
    fontSize: 12,
    fontFamily:
      'system-ui, -apple-system, BlinkMacSystemFont, "PingFang SC", "Microsoft YaHei", sans-serif',
  },
  colors: {
    ...denseGridTheme.colors,
    background: "#ffffff",
    headerBackground: "#f6f8fb",
    headerText: "#33405b",
    text: "#283650",
    gridLine: "#e8edf4",
    gridLineStrong: "#dde5ef",
    selectionBorder: "#0875ff",
    // DOM选区浮层覆盖在Canvas文字上方，必须保留透明度。
    selectionBg: "rgba(8, 117, 255, 0.09)",
    readonlyCellText: "#34415b",
  },
  cell: { ...denseGridTheme.cell, headerTextAlign: "center" as const },
  // 工程工作台自行展示空类说明，不使用组件默认插画及重复文案。
  emptyState: {
    ...denseGridTheme.emptyState,
    layers: [],
    title: "",
    subtitle: "",
  },
};
export const categories: [Category, string][] = [
  ["measurement", "遥测"],
  ["indication", "遥信"],
  ["control", "遥控"],
  ["adjustment", "遥调"],
];
/** 点表结构跟随当前模型引用；确认值只来自内容及通信配置一致的应用快照。 */
export function resolveUnitSheet(catalog: Catalog | undefined, endpoint: EndpointConfig, unit: UnitConfig, matchesByEndpoint?: Record<string,Record<string,boolean>>) {
  const model = catalog?.models.find(item => item.id === unit.model.id);
  const applied = catalog?.applied.find(item => item.id === endpoint.id);
  const appliedUnit = applied?.units.find(item => item.id === unit.id);
  const values = catalog?.values[unit.id];
  const matches = !!model && !!appliedUnit && !!matchesByEndpoint?.[endpoint.id]?.[unit.id];
  return { model, values: matches ? values : undefined };
}

export interface PendingRuntimeEdit {point:CollectionModel["points"][number];text:string;source:RuntimeDataSource;row:number;}
function viewKey(identity:string,category:Category,model:CollectionModel|undefined,role:Role){
 return JSON.stringify([identity,category,model?.id,model?.points.map(p=>role==="master"?[p.id,p.category,p.area,p.address,p.valueType,p.byteOrder,p.wordOrder,p.scale]:[p.id,p.category]),role]);
}
export class SheetViews {
  private views = new Map<string, RuntimeDataSource>();
  private listeners=new Set<()=>void>();
  private revision=0;
  subscribe=(listener:()=>void)=>{this.listeners.add(listener);return()=>{this.listeners.delete(listener);};};
  snapshot=()=>this.revision;
  private changed=()=>{this.revision++;for(const listener of [...this.listeners])listener();};
  pending(identity:string,model:CollectionModel):PendingRuntimeEdit[]{
   return categories.flatMap(([category])=>{
    if(category!=="measurement"&&category!=="indication")return [];
    const source=this.views.get(viewKey(identity,category,model,"slave"));
    if(!source)return [];
    const points=model.points.filter(point=>point.category===category);
    return source.pendingRows().flatMap(row=>points[row]?[{point:points[row]!,text:String(source.getCell(row,"draft")??""),source,row}]:[]);
   });
  }
  pendingMaster(identity:string,model:CollectionModel):PendingRuntimeEdit[]{
    return categories.flatMap(([category])=>{
      if(category!=="control"&&category!=="adjustment")return [];
      const source=this.views.get(viewKey(identity,category,model,"master"));if(!source)return [];
      const points=model.points.filter(p=>p.category===category);
      return source.pendingRows().flatMap(row=>points[row]?[{point:points[row]!,text:String(source.getCell(row,"draft")??""),source,row}]:[]);
    });
  }
  discardMaster(identity:string){for(const key of [...this.views.keys()]){const parts=JSON.parse(key);if(parts[0]===identity&&parts.at(-1)==="master")this.views.delete(key);}this.changed();}
  acknowledge(edits:PendingRuntimeEdit[]){for(const edit of edits)edit.source.acknowledge(edit.row,edit.text);}

  get(
    identity: string,
    category: Category,
    model: CollectionModel | undefined,
    values?: UnitValues,
    role: Role = "slave",
    metadata?:Record<string,MasterPoint>,
  ) {
    // 已应用模型快照变化时新建视图，普通查询／分类切换不覆盖草稿。
    const key = viewKey(identity,category,model,role);
    const acquisition = category === "measurement" || category === "indication";
    const editable = role === "slave" ? acquisition : !acquisition;
    let data = this.views.get(key);
    if (!data) {
      data = new RuntimeDataSource({
        schema: {
          fields: [
            { id: "name", name: "点名", type: "readonly", width: 140 },
            { id: "area", name: "数据区", type: "readonly", width: 90 },
            { id: "address", name: "零基地址", type: "readonly", width: 80 },
            { id: "valueType", name: "类型", type: "readonly", width: 80 },
            { id: "scale", name: "倍率", type: "readonly", width: 65 },
            { id: "unit", name: "单位", type: "readonly", width: 65 },
            {
              id: "draft",
              name:
                role === "slave"
                  ? acquisition
                    ? "置数草稿"
                    : "控制观测值"
                  : acquisition
                    ? "采集结果（原始）"
                    : "待发值（原始）",
              type: editable ? "text" : "readonly",
              width: 110,
            },
            {
              id: "confirmed",
              name: role === "master" ? "最近读取值（原始）" : "当前应用值",
              type: "readonly",
              width: 110,
            },
            ...(role==="master"?[
              {id:"quality",name:"质量",type:"readonly",width:90},
              {id:"updated",name:"成功更新时间",type:"readonly",width:145},
              {id:"result",name:"下发／回验结果",type:"readonly",width:230},
              {id:"error",name:"读取错误",type:"readonly",width:220},
            ]:[]),
          ],
        },
        rows: (model?.points ?? [])
          .filter((point) => point.category === category)
          .map((point) => ({
            id: point.id,
            name: point.name,
            area: point.area,
            address: point.address,
            valueType: point.valueType,
            scale: point.scale,
            unit: point.unit,
            draft: editable
              ? role === "master"
                ? ""
                : String((values?.raw[point.id] ?? point.initial).value)
              : values?.raw[point.id]
                ? String(values.raw[point.id].value)
                : null,
            confirmed: values?.raw[point.id]
              ? String(values.raw[point.id].value)
              : null,
          })),
      },this.changed);
      this.views.set(key, data);
    }
    // 真实应用确认只更新只读列，不覆盖用户尚未提交的运行值草稿。
    for (const [index, point] of (model?.points ?? [])
      .filter((point) => point.category === category)
      .entries()) {
      for(const field of ["name","area","address","valueType","scale","unit"] as const) {
        if(data.getCell(index,field)!==point[field]) data.updateCell(index,field,point[field]);
      }
      const confirmed = values?.raw[point.id]
        ? String(values.raw[point.id].value)
        : null;
      if (data.getRows(index, index)[0]?.confirmed !== confirmed)
        data.updateCell(index, "confirmed", confirmed);
      if(role==="master"){
        const meta=metadata?.[point.id];
        const labels:Record<string,string>={unread:"未读取",valid:"有效",stale:"过期／待重读",failed:"读取失败",disconnected:"通信中断"};
        const fields={quality:labels[meta?.quality??"unread"]??"未读取",updated:meta?.unixMs?new Date(Number(meta.unixMs)).toLocaleString("zh-CN",{hour12:false}):"",result:meta?.writeResult??"",error:meta?.error??""};
        for(const [field,text]of Object.entries(fields))if(data.getCell(index,field)!==text)data.updateCell(index,field,text);
      }
      if (editable && role === "slave" && confirmed !== null) data.syncActual(index,confirmed);
      if (!editable && data.getCell(index, "draft") !== confirmed)
        data.syncActual(index,confirmed??"");
    }
    return data;
  }
}
