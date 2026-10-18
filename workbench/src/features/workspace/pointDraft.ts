/** 模型唯一编辑草稿；四遥视图只做过滤，事务按稳定点身份写回。 */
import type {
  Category,
  CollectionModel,
  DraftPreview,
} from "../../shared/api/contracts";
export type InputRow = Record<string, string>;
export interface DraftState {
  name: string;
  rows: InputRow[];
}
export const fields = [
  "name",
  "category",
  "area",
  "address",
  "valueType",
  "byteOrder",
  "wordOrder",
  "scale",
  "unit",
  "initialEngineering",
  "slaveWritable",
  "masterPeriodMs",
  "note",
] as const;
export const labels: Record<string, string> = {
  name: "点名",
  category: "四遥分类",
  area: "数据区",
  address: "零基地址",
  valueType: "类型",
  byteOrder: "字节序",
  wordOrder: "字序",
  scale: "倍率",
  unit: "单位",
  initialEngineering: "初始工程值",
  slaveWritable: "允许外部写入",
  masterPeriodMs: "采集周期 ms",
  note: "备注",
  width: "占用长度",
  raw: "原始值预览",
};
export const choices: Record<string, string[]> = {
  category: ["measurement", "indication", "control", "adjustment"],
  area: ["holding", "input", "coil", "discrete"],
  valueType: ["bool", "i16", "u16", "i32", "u32", "i64", "u64", "f32", "f64"],
  byteOrder: ["big", "little"],
  wordOrder: ["big", "little"],
  slaveWritable: ["false", "true"],
};
export const widths: Record<string, number> = {
  bool: 1,
  i16: 1,
  u16: 1,
  i32: 2,
  u32: 2,
  f32: 2,
  i64: 4,
  u64: 4,
  f64: 4,
};
export function defaultRow(
  category: Category,
  id: string = crypto.randomUUID(),
): InputRow {
  return {
    id,
    name: "新点",
    category,
    area: "holding",
    address: "0",
    valueType: "u16",
    byteOrder: "big",
    wordOrder: "big",
    scale: "1",
    unit: "",
    initialEngineering: "0",
    slaveWritable: "false",
    masterPeriodMs: "1000",
    note: "",
  };
}
export function modelRows(model: CollectionModel): InputRow[] {
  return model.points.map((p) =>
    Object.fromEntries(
      ["id", ...fields].map((k) => [k, String(p[k as keyof typeof p] ?? "")]),
    ),
  );
}
export class ModelDraft {
  private state: DraftState;
  private baseline: string;
  private past: DraftState[] = [];
  private future: DraftState[] = [];
  editCount = 0;
  preview: DraftPreview | null = null;
  selected: string | null = null;
  constructor(
    readonly id: string,
    name: string,
    rows: InputRow[],
    readonly notify: () => void = () => {},
    public persisted = true,
  ) {
    this.state = { name, rows: structuredClone(rows) };
    this.baseline = this.content();
  }
  get name() {
    return this.state.name;
  }
  get rows() {
    return this.state.rows;
  }
  get dirty() {
    return !this.persisted || this.content() !== this.baseline;
  }
  get canUndo() {
    return this.past.length > 0;
  }
  get canRedo() {
    return this.future.length > 0;
  }
  select(id: string | null) {
    if (this.selected === id) return;
    this.selected = id;
    this.notify();
  }
  content() {
    return JSON.stringify(this.state);
  }
  input() {
    return {
      id: this.id,
      name: this.name,
      points: structuredClone(this.rows),
    };
  }
  change(edit: (state: DraftState) => void) {
    const next = structuredClone(this.state);
    edit(next);
    const oldTypes = new Map(
      this.state.rows.map((row) => [row.id, row.valueType]),
    );
    for (const row of next.rows) {
      if (oldTypes.has(row.id) && oldTypes.get(row.id) !== row.valueType) {
        if ((widths[row.valueType!] ?? 1) === 1) row.wordOrder = "big";
        if (row.valueType === "bool") {
          row.byteOrder = "big";
          row.scale = "1";
        }
      }
    }
    if (JSON.stringify(next) === this.content()) return;
    this.past.push(this.state);
    if (this.past.length > 40) this.past.shift();
    this.state = next;
    this.future = [];
    this.editCount++;
    this.notify();
  }
  undo() {
    const previous = this.past.pop();
    if (previous) {
      this.future.push(this.state);
      this.state = previous;
      this.editCount++;
      this.notify();
    }
  }
  redo() {
    const next = this.future.pop();
    if (next) {
      this.past.push(this.state);
      this.state = next;
      this.editCount++;
      this.notify();
    }
  }
  saved(submitted: string) {
    this.persisted = true;
    this.baseline = submitted;
    this.notify();
  }
  discard() {
    this.state = JSON.parse(this.baseline);
    this.past = [];
    this.future = [];
    this.editCount++;
    this.notify();
  }
}
