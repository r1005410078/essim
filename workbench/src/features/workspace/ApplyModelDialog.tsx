import { WorkbenchDialog, DialogBody, DialogFooter } from "@essim/ui/components/workbench-dialog";
import {Button} from "@essim/ui/components/button";
import { useEffect, useRef, useState } from "react";
import type { ApplyPreview, EndpointConfig } from "../../shared/api/contracts";
import { WorkspaceStore, errorMessage } from "./model";

/** 确认以完整端点为范围的迁移；预览无副作用，提交仍由核心重新校验。 */
export function ApplyModelDialog({
  store,
  endpoint,
  close,
  pendingWrites=0,
  discardWrites,
}: {
  store: WorkspaceStore;
  endpoint: EndpointConfig;
  close: () => void;
  pendingWrites?:number;
  discardWrites?:()=>void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [discard,setDiscard]=useState(false);
  const [reset, setReset] = useState(false);
  const [preview, setPreview] = useState<ApplyPreview | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    let active = true;
    setPreview(null);
    setError("");
    if (!store.client.previewApply) {
      setError("应用预览服务未接入");
      return;
    }
    store.client
      .previewApply(endpoint.id, reset)
      .then((value) => {
        if (active) setPreview(value);
      })
      .catch((e) => {
        if (active) setError(errorMessage(e));
      });
    return () => {
      active = false;
    };
  }, [store, endpoint, reset]);
  return (
    <WorkbenchDialog ref={dialog} className="resource-dialog apply-dialog" onCancel={e=>{e.preventDefault();e.stopPropagation();if(!busy)close();}} title={<>应用端点配置</>} busy={busy} initialWidth={760}>
      <DialogBody>
      <div className="apply-identity"><strong>{endpoint.name}</strong><code>{endpoint.transport.kind==="tcp"?`${endpoint.transport.address}:${endpoint.transport.port}`:endpoint.transport.path}</code><span className="configuration-badge">{endpoint.units.length} 个 Unit</span></div>
      <p>应用范围为完整端点。现有连接会断开，应用完成后保持停止，需手动启动。</p>
      <table className="migration-table"><thead><tr><th>Unit／模型</th><th>保留</th><th>新增／恢复</th><th>移除</th><th>倍率变化</th></tr></thead><tbody>{preview?.endpoint.units.map(unit=>{
        const migration=preview.units[unit.id];const total=Object.keys(migration?.values.raw??{}).length;
        return <tr key={unit.id}><td>Unit {unit.station} · {unit.name}<div className="muted">{store.view?.catalog.models.find(model=>model.id===unit.model.id)?.name}</div></td><td>{Math.max(0,total-(migration?.initialized.length??0))}</td><td>{migration?.initialized.length??0}</td><td>{migration?.removed.length??0}</td><td>{migration?.scaleChanged.length??0}</td></tr>;
      })}</tbody></table>
      {!preview&&!error&&<p className="muted">正在校验并计算迁移…</p>}
      {preview&&<details className="migration-details"><summary>查看受影响点与工程值变化</summary>{preview.endpoint.units.map(unit=>{
        const migration=preview.units[unit.id];const model=store.view?.catalog.models.find(model=>model.id===unit.model.id);
        const previous=store.view?.catalog.values[unit.id]?.model;
        const label=(id:string)=>{const point=model?.points.find(p=>p.id===id)??previous?.points.find(p=>p.id===id);return point?`${point.name} · ${point.area}[${point.address}]`:id;};
        return <div key={unit.id}><strong>Unit {unit.station}</strong><ul>{migration?.initialized.map(id=><li key={`new:${id}`}>{label(id)}：{endpoint.role==="master"?"等待重新读取":"使用模型初始值"}</li>)}{migration?.removed.map(id=><li key={`removed:${id}`}>{label(id)}：移除</li>)}{migration?.engineeringChanged.map(change=><li key={change.point}>{label(change.point)}：{change.previous} → {change.next}（原始值不变）</li>)}</ul></div>;
      })}</details>}
      <label><input type="checkbox" checked={reset} disabled={busy} onChange={e=>setReset(e.target.checked)}/> 允许不兼容点恢复模型初始值</label>
      {endpoint.role==="master"&&<p className="muted">主站映射变化的采集结果失效，等待重新采集；不会向远端写入初值。</p>}
      {pendingWrites>0&&<label><input type="checkbox" checked={discard} onChange={e=>setDiscard(e.target.checked)}/> 明确丢弃此端点全部 {pendingWrites} 项待写草稿后应用</label>}
      {error && (
        <p role="alert" className="error">
          {error}
        </p>
      )}
      </DialogBody><DialogFooter>
        <Button variant="ghost" disabled={busy} onClick={close}>
          取消
        </Button>
        <Button
          disabled={!preview || busy || (pendingWrites>0&&!discard)}
          onClick={async () => {
            setBusy(true);
            setError("");
            try {
              await store.execute({
                kind: "apply",
                id: endpoint.id,
                resetIncompatible: reset,
              });
              discardWrites?.();
              close();
            } catch (e) {
              setError(errorMessage(e));
            } finally {
              setBusy(false);
            }
          }}
        >
          {busy?"正在应用…":"停止并应用配置"}
        </Button>
      </DialogFooter>
    </WorkbenchDialog>
  );
}
