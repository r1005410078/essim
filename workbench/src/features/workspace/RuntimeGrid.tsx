import {useEffect,useRef} from "react";
import {Grid,type DataSource,type GridEngineFrameSource} from "@zhiguang/novasheet-core";
import {canvas2dBackend} from "@zhiguang/novasheet-canvas2d";
import {readonlyTypes,sheetTheme} from "./sheets";
import {runtimeVisiblePoints} from "./slaveQueryPoints";

/** 复用NovaSheet公开Grid及Canvas后端，仅组装生命周期和可见行查询。 */
export function RuntimeGrid({data,onReader,onPoint,onSelectionReader}:{onSelectionReader?:(read:(()=>string[])|undefined)=>void;onPoint?:(point:string|undefined)=>void;data:DataSource;onReader:(read:(()=>string[])|undefined)=>void}){
 const host=useRef<HTMLDivElement>(null);
 const pointCallback=useRef(onPoint);pointCallback.current=onPoint;
 const selectionCallback=useRef(onSelectionReader);selectionCallback.current=onSelectionReader;
 const callback=useRef(onReader);callback.current=onReader;
 useEffect(()=>{
  if(!host.current)return;
  let engine:GridEngineFrameSource|undefined;
  const backend=canvas2dBackend();
  const grid=new Grid(host.current,{
   onSelectionChange:selection=>{
    const row=selection.activeCell?.rowIndex;
    const underlying=row===undefined?undefined:grid.getViewPipeline().getComposed().resolveUnderlyingRow?.(row)??row;
    const rows=underlying===undefined?undefined:data.getRows(underlying,underlying);
    pointCallback.current?.(Array.isArray(rows)&&rows[0]?.id!==undefined?String(rows[0].id):undefined);
   },
   data,theme:sheetTheme,cellTypes:readonlyTypes,
   backend:deps=>{engine=deps.engine;return backend(deps);},
   excelHeaders:true,excelWorkspace:true,rowHeaderCornerLabel:"",
   interactions:{contextMenu:false,reorder:false,resize:{rows:false,columns:true}},
  });
  selectionCallback.current?.(()=>{const range=grid.getSelection().selectedRange;if(!range)return [];if(range.endRow-range.startRow>=1024)throw Error("一次最多捕获1024个点");const rows=grid.getViewPipeline().getComposed().getRows(range.startRow,range.endRow);return Array.isArray(rows)?rows.map(r=>String(r.id)):[];});
  callback.current(()=>{
   if(!engine)return [];
   const source=engine.getData();
   return runtimeVisiblePoints(engine.getViewport().snapshot().regions,row=>{
    const rows=source.getRows(row,row);
    return Array.isArray(rows)&&rows[0]?.id!==undefined?String(rows[0].id):undefined;
   });
  });
  return()=>{selectionCallback.current?.(undefined);callback.current(undefined);engine=undefined;grid.destroy();};
 },[data]);
 return <div ref={host} className="h-full w-full"/>;
}
