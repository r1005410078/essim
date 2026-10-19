import {EnumSelect} from "./EnumSelect";
import {ActionMenu} from "./ActionMenu";
import {
  Component,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import type { Grid } from "@zhiguang/novasheet-core";
import { PointGrid } from "./PointGrid";
import { DraftSource } from "./DraftSource";
import {
  Save,
  Plus,
  Trash2,
  Undo2,
  Redo2,
  FileInput,
} from "lucide-react";
import type {
  ApiError,
  Category,
  DraftPreview,
} from "../../shared/api/contracts";
import { WorkspaceStore, errorMessage } from "./model";
import {
  ModelDraft,
  choices,
  defaultRow,
  fields,
  labels,
  widths,
  type InputRow,
} from "./pointDraft";
import { categories, readonlyTypes, sheetTheme } from "./sheets";
import { FieldHelp } from "./FieldHelp";
import { ImportPoints } from "./ImportPoints";

class EditorBoundary extends Component<
  { children: ReactNode },
  { error: string }
> {
  state = { error: "" };
  static getDerivedStateFromError(error: unknown) {
    return { error: errorMessage(error) };
  }
  render() {
    return this.state.error ? (
      <div role="alert" className="error panel-note">
        点表编辑器初始化失败：{this.state.error}
      </div>
    ) : (
      this.props.children
    );
  }
}
export function PointEditor({
  store,
  id,
}: {
  store: WorkspaceStore;
  id: string;
}) {
  useSyncExternalStore(store.subscribe, store.snapshot);
  const draft = store.modelDraft(id);
  if (!draft) return <div className="empty">模型不存在。</div>;
  return (
    <EditorBoundary key={id}>
      <EditorBody store={store} draft={draft} />
    </EditorBoundary>
  );
}
function EditorBody({
  store,
  draft,
}: {
  store: WorkspaceStore;
  draft: ModelDraft;
}) {
  const [category, setCategory] = useState<Category>("measurement");
  const [preview, setPreview] = useState<DraftPreview | null>(null);
  const [errorPage, setErrorPage] = useState(0);
  const [error, setError] = useState("");
  const [importing, setImporting] = useState(false);
  const [busy, setBusy] = useState(false);
  const grid = useRef<Grid>(null);
  const marked = useRef<{ row: number; col: number }[]>([]);
  const source = useMemo(
    () => new DraftSource(draft, category),
    [draft, category],
  );
  useEffect(() => {
    source.sync(preview);
    grid.current?.clearHistory();
  }, [source, draft.editCount, preview]);
  useEffect(() => {
    let active = true;
    const timer = setTimeout(() => {
      store.client
        .previewPoints?.(draft.input())
        .then((result) => {
          if (active) {
            draft.preview = result;
            setPreview(result);
            setErrorPage(0);
            store.changed();
          }
        })
        .catch((e) => {
          if (active) setError(errorMessage(e));
        });
    }, 180);
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [draft, draft.editCount, store]);
  useEffect(() => {
    const api = grid.current;
    if (!api) return;
    for (const cell of marked.current)
      api.setFillColor(
        {
          startRow: cell.row,
          endRow: cell.row,
          startCol: cell.col,
          endCol: cell.col,
        },
        null,
      );
    marked.current = [];
    let active = true;
    void Promise.resolve().then(async () => {
      const view = api.getViewPipeline().getComposed();
      const rows = await view.getRows(0, view.getRowCount() - 1);
      if (!active || grid.current !== api) return;
      const positions = new Map(
        rows.map((row, index) => [String(row.id), index]),
      );
      for (const issue of preview?.errors.slice(
        errorPage * 50,
        errorPage * 50 + 50,
      ) ?? []) {
        const row = positions.get(issue.pointId ?? "") ?? -1;
        const col = fields.indexOf(issue.field as (typeof fields)[number]);
        if (row >= 0 && col >= 0) {
          api.setFillColor(
            { startRow: row, endRow: row, startCol: col, endCol: col },
            "#ffe2dd",
          );
          marked.current.push({ row, col });
        }
      }
    });
    return () => {
      active = false;
    };
  }, [source, preview, errorPage]);
  const task = async (action: () => Promise<unknown>) => {
    setBusy(true);
    setError("");
    try {
      await action();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };
  const save = () =>
    task(async () => {
      source.flush();
      await store.saveModelDraft(draft.id);
    });
  const add = () =>
    draft.change((s) => {
      const row = defaultRow(category);
      row.address = String(
        Math.max(
          0,
          ...s.rows
            .filter((r) => r.area === "holding")
            .map((r) => Number(r.address) + (widths[r.valueType!] ?? 1)),
        ),
      );
      s.rows.push(row);
      draft.selected = row.id!;
    });
  const focusError = (issue: ApiError) => {
    const row = draft.rows.find((r) => r.id === issue.pointId);
    if (!row) return;
    setCategory(row.category as Category);
    draft.selected = row.id!;
    store.changed();
    requestAnimationFrame(() => {
      const api = grid.current;
      if (!api) return;
      const view = api.getViewPipeline().getComposed();
      void Promise.resolve(view.getRows(0, view.getRowCount() - 1)).then(
        (rows) => {
          if (grid.current !== api) return;
          const index = rows.findIndex((item) => item.id === row.id);
          if (index >= 0) api.scrollToCell(index, issue.field ?? "name");
        },
      );
    });
  };
  const focusIssue=useRef(focusError);focusIssue.current=focusError;
  useEffect(()=>{store.focusModelIssue.set(draft.id,issue=>focusIssue.current(issue));return()=>{store.focusModelIssue.delete(draft.id);};},[store,draft.id]);
  return (
    <div
      className="panel column point-editor"
      onKeyDownCapture={(event) => {
        if (
          (event.metaKey || event.ctrlKey) &&
          event.key.toLowerCase() === "z"
        ) {
          event.preventDefault();
          event.stopPropagation();
          source.flush();
          event.shiftKey ? draft.redo() : draft.undo();
        }

      }}
    >
      <div className="toolbar document-actions model-actions">
        <button
          data-save-document={`model:${draft.id}`}
          title="保存模型（⌘S / Ctrl+S，本次会话）"
          aria-label="保存模型"
          disabled={busy}
          onClick={() => void save()}
        >
          <Save size={17} />保存模型
        </button>
        <button title="新增点" aria-label="新增点" onClick={add}>
          <Plus size={17} />新增点
        </button>
        <button
          title="删除选中点"
          aria-label="删除选中点"
          disabled={!draft.selected}
          onClick={() =>
            draft.change((s) => {
              s.rows = s.rows.filter((r) => r.id !== draft.selected);
              draft.selected = null;
            })
          }
        >
          <Trash2 size={16} />
        </button>
        <button
          title="撤销"
          aria-label="撤销"
          disabled={!draft.canUndo}
          onClick={() => {
            source.flush();
            draft.undo();
          }}
        >
          <Undo2 size={17} />
        </button>
        <button
          title="重做"
          aria-label="重做"
          disabled={!draft.canRedo}
          onClick={() => draft.redo()}
        >
          <Redo2 size={17} />
        </button>
        <span>
          {draft.name} ·{" "}
          {draft.dirty ? "未保存" : "已保存"}
        </span>
        <span className="spacer" />
        <button
          title="导入追加"
          aria-label="导入追加"
          onClick={() => setImporting(true)}
        >
          <FileInput size={17} />导入
        </button>
        <ActionMenu label="导出模型" disabled={!draft.persisted} items={["json","csv","xlsx"].map(format=>({label:format.toUpperCase(),action:()=>void task(async()=>{const done=await store.client.exportPointFile?.(draft.id,format);if(done===undefined)throw Error("文件服务未接入");})}))}/>
      </div>
      <details className="field-help-disclosure"><summary>字段说明</summary><div className="point-help-strip" aria-label="点表字段说明">{fields.map(field=><span key={field}>{labels[field]}<FieldHelp field={field}/></span>)}</div></details>
      {error && (
        <div role="alert" className="error panel-note">
          {error}
        </div>
      )}
      {!!preview?.errors.length&&<div className="configuration-notice error" role="alert">{preview.errors.length} 处模型问题 · 底部“问题”面板可查看全部；<button onClick={()=>focusError(preview.errors[0]!)}>定位首项</button></div>}
      <div className="sheet-area">
        <PointGrid
          key={category}
          api={grid}
          source={source}
          onError={setError}
          onSelection={(selection) => {
            const rowIndex = selection.activeCell?.rowIndex;
            if (rowIndex === undefined) return;
            const underlying =
              grid.current
                ?.getViewPipeline()
                .getComposed()
                .resolveUnderlyingRow?.(rowIndex) ?? rowIndex;
            const row = source.getRows(underlying, underlying)[0];
            const next =
              row && draft.rows.some((r) => r.id === row.id)
                ? String(row.id)
                : null;
            draft.select(next);
          }}
        />
      </div>
      <div className="category-tabs" role="tablist" aria-label="模型四遥分类">
        {categories.map(([value, label]) => (
          <button
            key={value}
            role="tab"
            aria-selected={value === category}
            className={value === category ? "active" : ""}
            onClick={() => {
              source.flush();
              setCategory(value);
            }}
          >
            {label}
          </button>
        ))}
        <span className="spacer" />
        <span className="muted">{draft.rows.length} 点 · 本次会话</span>
      </div>
      {importing && (
        <ImportPoints
          store={store}
          target={draft}
          close={() => setImporting(false)}
        />
      )}
    </div>
  );
}

export function ModelDetails({
  store,
  id,
  open,
}: {
  store: WorkspaceStore;
  id: string;
  open: (target: { kind: "endpoint"; id: string; unit: string }) => void;
}) {
  const draft = store.modelDraft(id);
  if (!draft) return null;
  const selected = draft.rows.find((r) => r.id === draft.selected);
  const references = [
    ...(store.view?.catalog.endpoints ?? []).flatMap((e) =>
      e.units
        .filter((u) => u.model.id === id)
        .map((u) => ({ endpoint: e, unit: u, applied: false })),
    ),
    ...(store.view?.catalog.applied ?? []).flatMap((e) =>
      e.units
        .filter((u) => u.model.id === id)
        .map((u) => ({ endpoint: e, unit: u, applied: true })),
    ),
  ];
  return (
    <div className="panel details inspector-panel">
      <div className="details-title">
        <strong>详细信息</strong>
      </div>
      <div className="details-subject">{draft.name}</div>
      <details open>
        <summary>模型</summary>
        <label className="property">
          <span>名称</span>
          <input
            aria-label="模型名称"
            value={draft.name}
            onChange={(e) =>
              draft.change((s) => {
                s.name = e.target.value;
              })
            }
          />
        </label>
        <div className="panel-note">保存仅限本次会话，不自动应用。</div>
      </details>
      <details open>
        <summary>选中点</summary>
        {selected ? (
          fields.map((field) => (
            <label className="property" key={field}>
              <span>
                {labels[field]}
                <FieldHelp field={field} />
              </span>
              <div>
                {choices[field] ? (
                  <EnumSelect label={labels[field]} value={selected[field]??""} options={choices[field]!.map(value=>[value,value] as const)} disabled={(selected.valueType==="bool"&&(field==="byteOrder"||field==="wordOrder"))||((widths[selected.valueType!]??1)===1&&field==="wordOrder")} onChange={value=>draft.change(state=>{state.rows.find(row=>row.id===selected.id)![field]=value;})}/>

                ) : field === "initialEngineering" &&
                  selected.valueType === "bool" ? (
                  <input
                    type="checkbox"
                    aria-label="初始开关"
                    checked={
                      selected.initialEngineering === "true" ||
                      selected.initialEngineering === "1"
                    }
                    onChange={(e) =>
                      draft.change((s) => {
                        s.rows.find(
                          (r) => r.id === selected.id,
                        )!.initialEngineering = String(e.target.checked);
                      })
                    }
                  />
                ) : (
                  <input
                    aria-label={labels[field]}
                    disabled={
                      selected.valueType === "bool" && field === "scale"
                    }
                    value={selected[field] ?? ""}
                    onChange={(e) =>
                      draft.change((s) => {
                        s.rows.find((r) => r.id === selected.id)![field] =
                          e.target.value;
                      })
                    }
                  />
                )}
              </div>
            </label>
          ))
        ) : (
          <div className="panel-note">选择点表行以编辑属性。</div>
        )}
      </details>
      {selected && (
        <details open>
          <summary>编码预览</summary>
          <div className="panel-note">
            原始值：
            {String(
              draft.preview?.previews.find((p) => p.pointId === selected.id)
                ?.raw.value ?? "待校验",
            )}
            <br />
            工程值：
            {draft.preview?.previews.find((p) => p.pointId === selected.id)
              ?.engineering ?? "待校验"}
            <br />
            {selected.valueType === "bool" ? "位映射：" : "寄存器："}
            {draft.preview?.previews
              .find((p) => p.pointId === selected.id)
              ?.registers.map((v) =>
                selected.valueType === "bool"
                  ? String(v)
                  : v.toString(16).padStart(4, "0").toUpperCase(),
              )
              .join(" · ") ?? "待校验"}
          </div>
        </details>
      )}
      <details open>
        <summary>模型引用</summary>
          <div className="model-references">
            <strong>
              {references.length} 处引用
            </strong>
            {references.map((ref, index) => (
              <button
                key={index}
                onClick={() =>
                  open({
                    kind: "endpoint",
                    id: ref.endpoint.id,
                    unit: ref.unit.id,
                  })
                }
              >
                {ref.endpoint.name} → Unit {ref.unit.station} ·{" "}
                {ref.applied ? "已应用" : "配置"}
              </button>
            ))}
          </div>
      </details>
    </div>
  );
}
