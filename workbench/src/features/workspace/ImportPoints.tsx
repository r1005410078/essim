import {EnumSelect} from "./EnumSelect";
import { useEffect, useMemo, useRef, useState } from "react";
import type {
  Category,
  PointFile,
  DraftInput,
} from "../../shared/api/contracts";
import { appendValidated } from "./pointImport";
import { WorkspaceStore, errorMessage } from "./model";
import { ModelDraft, defaultRow, fields, labels } from "./pointDraft";
import { categories } from "./sheets";

/** 导入预览独立于目标草稿；确认时始终重校验完整候选，失败不写入。 */
export function ImportPoints({
  store,
  target,
  close,
  created,
}: {
  store: WorkspaceStore;
  target?: ModelDraft;
  close: () => void;
  created?: (id: string) => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const [file, setFile] = useState<PointFile | null>(null);
  const [sheetIndex, setSheetIndex] = useState(0);
  const [page, setPage] = useState(0);
  const [mapping, setMapping] = useState<Record<string, string>>({});
  const [category, setCategory] = useState<Category | "">("");
  const [name, setName] = useState("导入模型");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const active = useRef(true);
  const reading = useRef<AbortController | null>(null);
  useEffect(() => {
    active.current = true;
    ref.current?.showModal();
    return () => {
      active.current = false;
      reading.current?.abort();
    };
  }, []);
  const sheet = file?.sheets[sheetIndex];
  const ids = useMemo<string[]>(
    () => sheet?.rows.map(() => crypto.randomUUID()) ?? [],
    [sheet?.headers, sheet?.rows.length],
  );
  useEffect(() => {
    if (!sheet) return;
    setMapping(
      Object.fromEntries(
        fields.map((field) => [
          field,
          String(
            sheet.headers.findIndex(
              (h) => h.trim() === field || h.trim() === labels[field],
            ),
          ),
        ]),
      ),
    );
    setPage(0);
  }, [sheet?.headers]);
  const openFile = async () => {
    setBusy(true);
    setError("");
    try {
      reading.current?.abort();
      reading.current = new AbortController();
      const value = await store.client.openPointFile?.(reading.current.signal);
      if (active.current && value) {
        setFile(value);
        setSheetIndex(0);
        setName(value.name.replace(/\.[^.]+$/, "") || "导入模型");
      }
    } catch (e) {
      if (active.current) setError(errorMessage(e));
    } finally {
      if (active.current) setBusy(false);
    }
  };
  const confirm = async () => {
    if (!sheet || !store.client.previewPoints) return;
    setBusy(true);
    setError("");
    try {
      if (Number(mapping.category ?? -1) < 0 && !category)
        throw Error("文件缺少四遥分类，请在预览中统一指定。");
      const rows = sheet.rows.map((cells, index) => {
        const row = defaultRow(
          (category || "measurement") as Category,
          ids[index]!,
        );
        for (const field of fields) {
          const column = Number(mapping[field] ?? -1);
          if (column >= 0) row[field] = cells[column] ?? "";
        }
        if (!row.category) row.category = category || "measurement";
        // 未提供权限列时，可写区默认允许；只读区遵循Modbus约束。
        if (Number(mapping.slaveWritable ?? -1) < 0)
          row.slaveWritable = row.area === "holding" || row.area === "coil" ? "true" : "false";
        return row;
      });
      const id = target?.id ?? crypto.randomUUID();
      const validate = async (input: DraftInput) => {
        const result = await store.client.previewPoints!(input);
        if (!result.model)
          throw Error(
            result.errors
              .slice(0, 50)
              .map((issue) => {
                const sourceRow = ids.indexOf(issue.pointId ?? "");
                return `${sourceRow >= 0 ? `文件第${sourceRow + 2}行 · ` : ""}${issue.field ? `${labels[issue.field] ?? issue.field}：` : ""}${issue.message}`;
              })
              .join("\n"),
          );
        return result;
      };
      if (target) {
        if (
          !(await appendValidated(target, rows, validate, () => active.current))
        )
          return;
        close();
        return;
      }
      await validate({
        id,
        name,
        points: rows,
      });
      if (!active.current) return;
      {
        const draft = new ModelDraft(id, name, rows, store.changed, false);
        store.modelDrafts.set(id, draft);
        store.changed();
        created?.(id);
      }
      close();
    } catch (e) {
      if (active.current) setError(errorMessage(e));
    } finally {
      if (active.current) setBusy(false);
    }
  };
  return (
    <dialog ref={ref} className="import-dialog" onCancel={close}>
      <h2>{target ? `追加到 ${target.name}` : "导入采集模型"}</h2>
      <p className="muted">
        预览及确认只修改草稿；不自动保存、不应用。导入生成新的点身份，不覆盖已有点。
      </p>
      <button type="button" onClick={() => void openFile()} disabled={busy}>
        选择 JSON / CSV / XLSX 文件
      </button>
      <EnumSelect label="下载点表模板" disabled={busy} value="" options={[["","下载空模板…"],["csv","CSV"],["xlsx","XLSX"],["json","JSON"]]} onChange={format=>{if(format){setError("");void store.client.exportPointTemplate?.(format).catch(e=>setError(errorMessage(e)));}}}/>

      {busy && <p role="status">正在读取或校验…</p>}
      {file && (
        <>
          {!target && (
            <label>
              模型名称
              <input
                aria-label="导入模型名称"
                value={name}
                disabled={busy}
                onChange={(e) => setName(e.target.value)}
              />
            </label>
          )}
          <label>
            工作表
            <EnumSelect label="导入工作表" value={String(sheetIndex)} disabled={busy} onChange={value=>setSheetIndex(Number(value))} options={file.sheets.map((sheet,index)=>[String(index),`${sheet.name} · ${sheet.rows.length} 行`] as const)}/>

          </label>
          <label>
            缺省四遥分类
            <EnumSelect label="缺省四遥分类" value={category} disabled={busy} onChange={value=>setCategory(value as Category)} options={[["","请选择（缺失时必选）"],...categories]}/>

          </label>
          <details open>
            <summary>列映射</summary>
            <div className="mapping-grid">
              {fields.map((field) => (
                <label key={field}>
                  {labels[field]}
                  <EnumSelect label={`${labels[field]}列映射`} disabled={busy||file.name.toLowerCase().endsWith(".json")} value={mapping[field]??"-1"} onChange={value=>setMapping({...mapping,[field]:value})} options={[["-1","未映射，使用缺省值"],...(sheet?.headers.map((header,index)=>[String(index),header||`第${index+1}列`] as const)??[])]}/>

                </label>
              ))}
            </div>
          </details>
          <div className="import-preview">
            <table>
              <thead>
                <tr>
                  {sheet?.headers.map((header, i) => (
                    <th key={i}>{header}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {sheet?.rows.slice(page * 20, page * 20 + 20).map((row, i) => (
                  <tr key={i}>
                    {row.map((cell, j) => (
                      <td key={j}>
                        <input
                          aria-label={`文件第${page * 20 + i + 2}行 ${sheet.headers[j]}`}
                          value={cell}
                          disabled={busy}
                          onChange={(e) => {
                            const value = e.target.value;
                            setFile((current) =>
                              current
                                ? {
                                    ...current,
                                    sheets: current.sheets.map((s, index) =>
                                      index !== sheetIndex
                                        ? s
                                        : {
                                            ...s,
                                            rows: s.rows.map((r, index) =>
                                              index !== page * 20 + i
                                                ? r
                                                : r.map((v, index) =>
                                                    index === j ? value : v,
                                                  ),
                                            ),
                                          },
                                    ),
                                  }
                                : current,
                            );
                          }}
                        />
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className="muted">
            每页20行，可修正源文本；确认时校验全部{sheet?.rows.length ?? 0}行。
            <button
              disabled={page === 0 || busy}
              onClick={() => setPage(page - 1)}
            >
              上一页
            </button>
            <span> 第 {page + 1} 页 </span>
            <button
              disabled={busy || (page + 1) * 20 >= (sheet?.rows.length ?? 0)}
              onClick={() => setPage(page + 1)}
            >
              下一页
            </button>
          </p>
        </>
      )}
      {error && (
        <pre className="error" role="alert">
          {error}
        </pre>
      )}
      <div className="dialog-actions">
        <button onClick={close}>取消</button>
        <button
          disabled={!file || busy}
          className="primary"
          onClick={() => void confirm()}
        >
          校验并导入草稿
        </button>
      </div>
    </dialog>
  );
}
