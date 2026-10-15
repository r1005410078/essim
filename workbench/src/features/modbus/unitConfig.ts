// 本阶段产品范围为 1–247；默认值和范围同样由后端独立校验。
export function parseNewUnitId(value: string, existing: readonly number[]): number {
  const unitId = Number(value);
  if (!/^\d+$/.test(value.trim()) || !Number.isInteger(unitId) || unitId < 1 || unitId > 247) {
    throw new Error('Unit ID 必须为 1–247 的整数');
  }
  if (existing.includes(unitId)) throw new Error('Unit ID 已存在');
  return unitId;
}
export function nextUnitId(existing: readonly number[]): number | undefined {
  for (let id = 1; id <= 247; id++) if (!existing.includes(id)) return id;
  return undefined;
}
