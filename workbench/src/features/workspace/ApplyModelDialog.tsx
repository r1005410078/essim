import { useEffect, useRef, useState } from "react";
import type { ApplyPreview, EndpointConfig } from "../../shared/api/contracts";
import { WorkspaceStore, errorMessage } from "./model";

/** 确认以完整端点为范围的迁移；预览无副作用，提交仍由核心重新校验。 */
export function ApplyModelDialog({
  store,
  endpoint,
  close,
}: {
  store: WorkspaceStore;
  endpoint: EndpointConfig;
  close: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [reset, setReset] = useState(false);
  const [preview, setPreview] = useState<ApplyPreview | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    dialog.current?.showModal();
  }, []);
  useEffect(() => {
    let active = true;
    setPreview(null);
    setError("");
    if (!store.client.previewApply) {
      setError("应用预览服务未接入");
      return;
    }
    store.client
      .previewApply(endpoint.id, endpoint.revision, reset)
      .then((value) => {
        if (active) setPreview(value);
      })
      .catch((e) => {
        if (active) setError(errorMessage(e));
      });
    return () => {
      active = false;
    };
  }, [store, endpoint.id, endpoint.revision, reset]);
  return (
    <dialog ref={dialog} className="resource-dialog" onCancel={close}>
      <h2>应用端点配置</h2>
      <p>
        范围：{endpoint.name} 的完整{" "}
        {endpoint.transport.kind === "tcp" ? "TCP 监听端点" : "RTU 串口通道"}
        ，共 {endpoint.units.length} 个 Unit。应用后保持停止，等待手动启动。
      </p>
      <p className="muted">
        当前没有生产执行器，本次只应用会话内配置和值，不产生实际通信。
      </p>
      <label>
        <input
          type="checkbox"
          checked={reset}
          disabled={busy}
          onChange={(e) => setReset(e.target.checked)}
        />{" "}
        明确允许不兼容点恢复模型初值
      </label>
      {preview &&
        preview.endpoint.units.map((unit) => {
          const migration = preview.units[unit.id];
          return (
            <div className="panel-note" key={unit.id}>
              <strong>
                Unit {unit.station} · {unit.name}
              </strong>{" "}
              → 模型 v{unit.model.version}
              <br />
              {endpoint.role === "master" ? "新增／失效" : "新增／恢复"}{" "}
              {migration?.initialized.length ?? 0} 点，移除{" "}
              {migration?.removed.length ?? 0} 点，倍率变化{" "}
              {migration?.scaleChanged.length ?? 0} 点；
              {endpoint.role === "master"
                ? "映射变化的结果失效，等待重新采集；不向远端写入初值。"
                : "其余保留原始数值。"}
              {migration?.engineeringChanged.map((change) => (
                <div key={change.point}>
                  {change.point}：{change.previous} → {change.next}（
                  {endpoint.role === "master"
                    ? "新倍率示意，采集结果将失效"
                    : "原始值不变"}
                  ）
                </div>
              ))}
            </div>
          );
        })}
      {error && (
        <p role="alert" className="error">
          {error}
        </p>
      )}
      <div className="dialog-actions">
        <button disabled={busy} onClick={close}>
          取消
        </button>
        <button
          className="primary"
          disabled={!preview || busy}
          onClick={async () => {
            setBusy(true);
            setError("");
            try {
              await store.execute({
                kind: "apply",
                id: endpoint.id,
                expectedRevision: endpoint.revision,
                resetIncompatible: reset,
              });
              close();
            } catch (e) {
              setError(errorMessage(e));
            } finally {
              setBusy(false);
            }
          }}
        >
          确认应用
        </button>
      </div>
    </dialog>
  );
}
