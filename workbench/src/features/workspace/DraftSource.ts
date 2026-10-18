import { SnapshotDataSource } from "./SnapshotDataSource";
import { type CellValue } from "@zhiguang/novasheet-core";
import type { Category, DraftPreview } from "../../shared/api/contracts";
import {
  ModelDraft,
  choices,
  defaultRow,
  fields,
  labels,
  widths,
  type InputRow,
} from "./pointDraft";
/** 组件仅维护可见行缓存；所有业务编辑在微任务末合并写入单一草稿。 */
export class DraftSource extends SnapshotDataSource {
  private blankIds = Array.from({ length: 30 }, () => crypto.randomUUID());
  private pending = new Map<string, InputRow>();
  private queued = false;
  private viewIds: string[] = [];
  private canonical = new Map<string, InputRow>();
  constructor(
    readonly draft: ModelDraft,
    readonly category: Category,
  ) {
    super({
      schema: {
        fields: [
          ...fields.map((id) => ({
            id,
            name: labels[id]!,
            type: choices[id] ? "singleSelect" : "text",
            width: id === "name" ? 150 : id === "note" ? 200 : 120,
            options: choices[id] ? { choices: choices[id] } : undefined,
          })),
          { id: "width", name: labels.width!, type: "readonly", width: 80 },
          { id: "raw", name: labels.raw!, type: "readonly", width: 160 },
        ],
      },
      rows: [],
    });
    this.sync();
  }
  sync(preview?: DraftPreview | null) {
    if (this.queued) return;
    this.canonical = new Map(this.draft.rows.map((row) => [row.id!, row]));
    const previews = new Map(
      preview?.previews.map((p) => [p.pointId, p]) ?? [],
    );
    // 当前编辑视图按稳定ID保留行槽；修改分类不立即迁移，手动切页后重新过滤。
    const blanks = new Set<string>(this.blankIds);
    this.viewIds = this.viewIds.filter(id => this.canonical.has(id) || blanks.has(id));
    const known = new Set(this.viewIds);
    const added = this.draft.rows
      .filter(row => row.category === this.category && !known.has(row.id!))
      .map(row => row.id!);
    const firstBlank = this.viewIds.findIndex(id => blanks.has(id));
    this.viewIds.splice(firstBlank < 0 ? this.viewIds.length : firstBlank, 0, ...added);
    this.viewIds.push(...this.blankIds.filter(id => !known.has(id)));
    super.setRows(this.viewIds.map(id => {
      const row = this.canonical.get(id);
      return row ? {
        ...row,
        id,
        width: String(widths[row.valueType!] ?? ""),
        raw: String(previews.get(id)?.raw.value ?? ""),
      } : { id };
    }));
  }
  ensureCapacity(count: number) {
    const extra = Math.max(0, count - this.getRowCount());
    if (this.draft.rows.length + extra > 100000)
      throw Error("粘贴超过100000点保护上限");
    if (extra) {
      this.blankIds.push(
        ...Array.from({ length: extra }, () => crypto.randomUUID()),
      );
      this.sync();
    }
  }
  canEdit(index: number, field: string) {
    if (!fields.includes(field as (typeof fields)[number])) return false;
    const visible = this.getRows(index, index)[0];
    if (!visible) return false;
    const row = this.pending.get(String(visible.id)) ?? visible;
    return !(
      (row.valueType === "bool" &&
        ["scale", "byteOrder", "wordOrder"].includes(field)) ||
      ((widths[String(row.valueType)] ?? 1) === 1 && field === "wordOrder")
    );
  }
  override updateCell(index: number, field: string, value: CellValue) {
    if (!this.canEdit(index, field)) return;
    const visible = this.getRows(index, index)[0];
    if (!visible) return;
    const id = String(visible.id);
    const existing = this.canonical.get(id);
    const row =
      this.pending.get(id) ??
      structuredClone(existing ?? defaultRow(this.category, id));
    if (
      (row.valueType === "bool" &&
        ["scale", "byteOrder", "wordOrder"].includes(field)) ||
      ((widths[row.valueType!] ?? 1) === 1 && field === "wordOrder")
    )
      return;
    row[field] = String(value ?? "");
    this.pending.set(id, row);
    super.updateCell(index, field, value);
    if (!this.queued) {
      this.queued = true;
      queueMicrotask(() => this.flush());
    }
  }
  override updateCellByUnderlyingRow(
    index: number,
    field: string,
    value: CellValue,
  ) {
    this.updateCell(index, field, value);
  }
  flush() {
    if (!this.queued) return;
    const changes = this.pending;
    this.pending = new Map();
    this.queued = false;
    this.draft.change((state) => {
      const positions = new Map(
        state.rows.map((row, index) => [row.id, index]),
      );
      for (const [id, row] of changes) {
        const index = positions.get(id);
        if (index === undefined) state.rows.push(row);
        else state.rows[index] = row;
      }
    });
    this.blankIds = this.blankIds.filter((id) => !changes.has(id)).slice(0, 30);
    while (this.blankIds.length < 30) this.blankIds.push(crypto.randomUUID());
    this.sync();
  }
}
