/** 点表网格在最后一条有效数据后再留出的空行数。 */
export const POINT_SHEET_SPARE_ROWS = 1000;

const textFields = ["name", "address", "type", "unit", "access"];

export function isBlankPointRow(row) {
  if (!row) return true;
  if (textFields.some((id) => String(row[id] ?? "").trim() !== "")) return false;
  if (row.scale !== "" && row.scale != null) return false;
  if (row.value !== "" && row.value != null) return false;
  return true;
}

/** 有效数据行数加空行余量。中间的空行保留，只补齐或裁掉末尾空行。 */
export function planSpareRows(rows, spare = POINT_SHEET_SPARE_ROWS) {
  let last = -1;
  for (let i = 0; i < rows.length; i += 1) {
    if (!isBlankPointRow(rows[i])) last = i;
  }
  const desired = last + 1 + spare;
  if (rows.length < desired) {
    return { desired, insertAt: rows.length, insert: desired - rows.length, deleteIds: [] };
  }
  if (rows.length > desired) {
    const deleteIds = [];
    for (let i = desired; i < rows.length; i += 1) {
      if (!isBlankPointRow(rows[i])) break;
      deleteIds.push(i);
    }
    return { desired, insertAt: rows.length, insert: 0, deleteIds };
  }
  return { desired, insertAt: rows.length, insert: 0, deleteIds: [] };
}

let adjusting = false;

export function ensureSpareRows(data, spare = POINT_SHEET_SPARE_ROWS) {
  if (adjusting) return;
  const count = data.getRowCount();
  const rows = count === 0 ? [] : data.getRows(0, count - 1);
  const plan = planSpareRows(rows, spare);
  if (plan.insert === 0 && plan.deleteIds.length === 0) return;
  adjusting = true;
  try {
    if (plan.insert > 0) data.insertRows(plan.insertAt, plan.insert);
    if (plan.deleteIds.length > 0) data.deleteRows(plan.deleteIds);
  } finally {
    adjusting = false;
  }
}
