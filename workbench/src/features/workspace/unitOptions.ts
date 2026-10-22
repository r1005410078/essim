import type { UnitConfig } from "../../shared/api/contracts";
export type SelectableUnit = Pick<UnitConfig,"id"|"station"|"name"|"enabled">;
export function describeUnit(unit: SelectableUnit) {
  const number = String(unit.station).padStart(2,"0");
  const name = unit.name.trim();
  const isDefault = !name || new RegExp(`^(?:unit|单元)\\s*0*${unit.station}$`,"i").test(name);
  return {number,name:isDefault?null:name,label:isDefault?`Unit ${number}`:`Unit ${number} · ${name}`};
}
export function filterUnits<T extends SelectableUnit>(units: readonly T[], query: string): T[] {
  const search = query.trim().toLocaleLowerCase();
  return units.filter(unit => `${unit.name} ${describeUnit(unit).label}`.toLocaleLowerCase().includes(search));
}
