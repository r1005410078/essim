/** 单层覆盖的编辑投影；协议与精确数值校验始终交由核心。 */
import type { CollectionModel, PointDefinition, UnitConfig, UnitOverrides, OverrideField, OverrideSelection } from '../../shared/api/contracts';
import { ModelDraft, modelRows, fields } from './pointDraft';
export const emptyOverrides = (): UnitOverrides => ({ pointOverrides: {}, addedPoints: [], removedPointIds: [] });
export function normalizedOverrides(value?: UnitOverrides): UnitOverrides { return { ...emptyOverrides(), ...value }; }
export function effectiveRows(base: CollectionModel, unit: UnitConfig) {
    const o = normalizedOverrides(unit.overrides);
    const hidden = new Set(o.removedPointIds);
    return [...modelRows(base).filter(p => !hidden.has(p.id!)).map(p => ({ ...p, ...Object.fromEntries(Object.entries(o.pointOverrides[p.id!] ?? {}).map(([k, v]) => [k, String(v)])) })), ...modelRows({ ...base, points: o.addedPoints })];
}
export function buildOverrides(base: CollectionModel, previous: UnitOverrides | undefined, model: CollectionModel, inherit: string[] = []): UnitOverrides {
    const original = normalizedOverrides(previous);
    const result = emptyOverrides();
    const bases = new Map(base.points.map(p => [p.id, p]));
    const retained = new Set(model.points.map(p => p.id));
    const cleared = new Set(inherit);
    for (const p of base.points)
        if (!retained.has(p.id))
            result.removedPointIds.push(p.id);
    for (const point of model.points) {
        const parent = bases.get(point.id);
        if (!parent) {
            result.addedPoints.push(point);
            continue;
        }
        const patch: Record<string, unknown> = {};
        for (const f of fields) {
            if (JSON.stringify(point[f]) !== JSON.stringify(parent[f]) || (Object.hasOwn(original.pointOverrides[point.id] ?? {}, f) && !cleared.has(`${point.id}:${f}`)))
                patch[f] = point[f];
        }
        if (Object.keys(patch).length)
            result.pointOverrides[point.id] = patch;
    }
    return result;
}
export function allOverrideSelection(value?: UnitOverrides): OverrideSelection { const o = normalizedOverrides(value); return { fields: Object.fromEntries(Object.entries(o.pointOverrides).map(([id, p]) => [id, Object.keys(p) as OverrideField[]])), added: o.addedPoints.map(p => p.id), removed: o.removedPointIds }; }
export function overrideCount(value?: UnitOverrides) { const o = normalizedOverrides(value); return Object.values(o.pointOverrides).reduce((n, p) => n + Object.keys(p).length, 0) + o.addedPoints.length + o.removedPointIds.length; }
export class UnitPointDraft extends ModelDraft {
    readonly base: CollectionModel;
    readonly sourceUnit: UnitConfig;
    constructor(readonly endpointId: string, unit: UnitConfig, base: CollectionModel, notify: () => void) { super(base.id, base.name, effectiveRows(base, unit), notify); this.base = structuredClone(base); this.sourceUnit = structuredClone(unit); }
    changedBase(base: CollectionModel | undefined) { return !base || JSON.stringify(base) !== JSON.stringify(this.base); }
    restore(selection?: OverrideSelection) {
        const baseIds = new Set(this.base.points.map(p => p.id));
        const target = selection ?? { fields: Object.fromEntries(this.base.points.map(p => [p.id, [...fields]])), added: this.rows.filter(p => !baseIds.has(p.id!)).map(p => p.id!), removed: this.base.points.map(p => p.id) };
        const rows = new Map(modelRows(this.base).map(p => [p.id!, p]));
        this.change(state => {
            state.inheritedFields ??= [];
            for (const [id, fs] of Object.entries(target.fields)) {
                const row = state.rows.find(p => p.id === id), base = rows.get(id);
                if (!base)
                    continue;
                for (const f of fs) {
                    if (row)
                        row[f] = base[f]!;
                    state.inheritedFields.push(`${id}:${f}`);
                }
            }
            state.rows = state.rows.filter(p => !target.added.includes(p.id!));
            for (const id of target.removed) {
                if (!state.rows.some(p => p.id === id) && rows.has(id))
                    state.rows.push(structuredClone(rows.get(id)!));
            }
            const order = new Map(this.base.points.map((p, i) => [p.id, i]));
            state.rows.sort((a, b) => (order.get(a.id!) ?? Infinity) - (order.get(b.id!) ?? Infinity));
        });
    }
    overrides(model: CollectionModel) { return buildOverrides(this.base, this.sourceUnit.overrides, model, this.inheritedFields); }
}
