import {
  applyPaste,
  computePasteTarget,
  parseTsvToCells,
  type DataSource,
  type GridSelection,
  type MutableDataSource,
  type FieldType,
} from "@zhiguang/novasheet-core";
import { DraftSource } from "./DraftSource";

/** 接收系统paste事件的可信文本，复用NovaSheet格式与类型规则；行映射先冻结。 */
export async function pastePointText(
  source: DraftSource,
  getView: () => DataSource,
  selection: GridSelection,
  text: string,
  resolvedType?: (row: number, col: number) => FieldType,
) {
  if (!selection.activeCell || !selection.selectedRange) return 0;
  if (new TextEncoder().encode(text).length > 16 * 1024 * 1024)
    throw Error("粘贴内容超过16MiB");
  const schema = getView().getSchema();
  const fieldIds = schema.fields.map((f) => f.id);
  const cells =
    text === ""
      ? [[""]]
      : parseTsvToCells(
          text,
          fieldIds.slice(selection.activeCell.colIndex),
          schema,
        );
  if (!cells.length) return 0;
  source.ensureCapacity(selection.activeCell.rowIndex + cells.length);
  // 公开数据源事件在微任务中重建Grid管线，随后取得最新可见行映射。
  await Promise.resolve();
  const view = getView();
  const target = computePasteTarget(
    selection.activeCell,
    selection.selectedRange,
    cells.length,
    cells[0]?.length ?? 0,
    { rowCount: view.getRowCount(), colCount: fieldIds.length },
  );
  const rows = new Map<number, number>();
  for (let r = target.startRow; r <= target.endRow; r++)
    rows.set(r, view.resolveUnderlyingRow?.(r) ?? r);
  const data: MutableDataSource = {
    getRowCount: () => view.getRowCount(),
    getSchema: () => schema,
    subscribe: (listener) => source.subscribe(listener),
    getRows: (start, end) =>
      Array.from(
        { length: Math.max(0, end - start + 1) },
        (_, i) =>
          source.getRows(
            rows.get(start + i) ?? start + i,
            rows.get(start + i) ?? start + i,
          )[0] ?? {},
      ),
    getCell: (r, f) => source.getCell(rows.get(r) ?? r, f),
    updateCell: (r, f, v) => source.updateCell(rows.get(r) ?? r, f, v),
  };
  let skipped = 0;
  applyPaste(
    {
      cells,
      sourceFieldIds: fieldIds.slice(selection.activeCell.colIndex),
      typed: false,
    },
    target,
    schema,
    fieldIds,
    data,
    (issues) => {
      skipped = issues.length;
    },
    undefined,
    resolvedType ? (r, c) => resolvedType(r, c) : undefined,
    (r, _c, f) => source.canEdit(rows.get(r) ?? r, f),
  );
  source.flush();
  return skipped;
}
