import type { UnitSnapshot } from '../../shared/api/contracts';
import { InMemoryDataSource, denseGridTheme } from '@zhiguang/novasheet-core';
// The bundled React adapter does not forward isCellEditable. Use its supported
// cellTypes contract to keep model and confirmed values read-only on all paths.
export const socCellTypes = { readonly: { editable: false, serializeClipboard: (value: unknown) => value == null ? '' : String(value) } };
export const socSheetTheme = { ...denseGridTheme, metrics: { ...denseGridTheme.metrics, rowHeaderWidth: 38 } };
export function createSocSheet() {
  return new InMemoryDataSource({
    schema: { fields: [
      { id: 'name', name: '点名', type: 'readonly', width: 130 },
      { id: 'address', name: '地址', type: 'readonly', width: 80 },
      { id: 'type', name: '数据类型', type: 'readonly', width: 90 },
      { id: 'scale', name: '倍率', type: 'readonly', width: 70 },
      { id: 'value', name: '运行值草稿', type: 'number', width: 110 },
      { id: 'applied', name: '已生效原始值', type: 'readonly', width: 115 },
      { id: 'engineering', name: '工程值', type: 'readonly', width: 90 },
    ] },
    rows: [{ id: 'soc', name: 'SOC', address: 100, type: 'UInt16', scale: '0.1%', value: 650, applied: null, engineering: '—' }],
  });
}
export function draftSoc(data: InMemoryDataSource): number {
  const row = data.getRows(0, 0)[0];
  if (row?.id !== 'soc') throw { code: 'POINT_NOT_FOUND', message: 'SOC 点不存在' };
  const value = row.value;
  if (typeof value !== 'number' || !Number.isInteger(value) || value < 0 || value > 1000) {
    throw { code: 'INVALID_VALUE', message: 'SOC 原始值必须为 0–1000 的整数' };
  }
  return value;
}

// 每个 Unit ID 拥有自己的表格草稿；只有后端确认值改变时才刷新，日志快照不覆盖编辑。
export class UnitSocSheets {
  private readonly sheets = new Map<string, ReturnType<typeof createSocSheet>>();
  private readonly applied = new Map<string, number>();
  get(unitId: number, transport: 'tcp' | 'rtu' = 'tcp') {
    const key = `${transport}:${unitId}`;
    let sheet = this.sheets.get(key);
    if (!sheet) { sheet = createSocSheet(); this.sheets.set(key, sheet); }
    return sheet;
  }
  sync(unit: UnitSnapshot, transport: 'tcp' | 'rtu' = 'tcp') {
    const key = `${transport}:${unit.unitId}`;
    if (this.applied.get(key) === unit.socRaw) return;
    const sheet = this.get(unit.unitId, transport);
    sheet.updateCell(0, 'value', unit.socRaw);
    sheet.updateCell(0, 'applied', unit.socRaw);
    sheet.updateCell(0, 'engineering', `${unit.socPercent.toFixed(1)}%`);
    this.applied.set(key, unit.socRaw);
  }
}
