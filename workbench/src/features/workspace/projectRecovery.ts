import type { WorkspaceStore } from './model';
import { ModelDraft, type DraftState } from './pointDraft';
import { UnitPointDraft } from './unitOverrides';
import type { CollectionModel, EndpointConfig, UnitConfig } from '../../shared/api/contracts';
interface ModelRecovery { id: string; persisted: boolean; baseline: string; content: string }
interface UnitRecovery { endpoint: string; unit: UnitConfig; base: CollectionModel; content: string }
export interface DraftRecovery { deferred?: DraftRecovery[]; format: 1; models: ModelRecovery[]; endpoints: { id: string; baseline: string; value: EndpointConfig }[]; units: UnitRecovery[] }
export function draftRecovery(store: WorkspaceStore): DraftRecovery {
  return { format: 1, models: [...store.modelDrafts].filter(([, d]) => d.dirty).map(([id, d]) => ({ id, persisted: d.persisted, baseline: d.savedContent(), content: d.content() })), endpoints: [...store.drafts].map(([id, value]) => ({ id, baseline: store.draftBaselines.get(id) ?? JSON.stringify(store.view?.catalog.endpoints.find(e => e.id === id)), value })), units: [...store.unitDrafts].filter(([, d]) => d.dirty).map(([, d]) => ({ endpoint: d.endpointId, unit: d.sourceUnit, base: d.base, content: d.content() })) };
}
export function recoverySets(value: unknown): DraftRecovery[] {
  const todo: unknown[] = [value], result: DraftRecovery[] = [], seen = new Set<string>(); let count = 0;
  while (todo.length) {
if (++count > 64) throw Error('恢复草稿分组超过限制'); const r = todo.shift() as DraftRecovery;
    if (!r || r.format !== 1 || !Array.isArray(r.models) || !Array.isArray(r.endpoints) || !Array.isArray(r.units)) throw Error('不支持的草稿恢复格式');
    const plain = { format: 1 as const, models: r.models, endpoints: r.endpoints, units: r.units }; const key = JSON.stringify(plain);
    if (!seen.has(key) && (r.models.length + r.endpoints.length + r.units.length) > 0) { seen.add(key); result.push(plain); }
    if (r.deferred) { if (!Array.isArray(r.deferred)) throw Error('恢复草稿分组不合法'); todo.push(...r.deferred); }
  } return result;
}
export function hasRecovery(value: unknown): value is DraftRecovery { try { return recoverySets(value).length > 0; } catch { return false; } }
/** 暂不恢复的旧输入与用户的新输入分别保留，不因关闭提示而丢失任一份。 */
export function combineRecovery(current: DraftRecovery, held: DraftRecovery | null): DraftRecovery {
  return { ...current, deferred: held ? recoverySets(held) : [] };
}

function state(content: string): DraftState {
  const s = JSON.parse(content) as DraftState;
  if (typeof s.name !== 'string' || !Array.isArray(s.rows) || s.rows.some(r => !r || typeof r !== 'object' || Object.values(r).some(v => typeof v !== 'string')) || s.inheritedFields?.some(v => typeof v !== 'string')) throw Error('恢复草稿内容不合法'); return s;
}
function restore(d: ModelDraft, content: string) { const s = state(content); d.change(v => { v.name = s.name; v.rows = s.rows; v.inheritedFields = s.inheritedFields; }); }
/** 冲突项保留恢复副本，不覆盖正式保存内容。 */
export async function restoreDraftRecovery(store: WorkspaceStore, recovery: DraftRecovery): Promise<string[]> {
  const conflicts: string[] = [];
  for (const r of recoverySets(recovery)) {
    for (const item of r.models) {
      state(item.content);
      const existing = store.modelDrafts.get(item.id); if (existing?.dirty) { if (existing.content() !== item.content) conflicts.push(item.id); continue; }
      if (store.view?.catalog.models.some(m => m.id === item.id)) {
        await store.loadTarget('model', item.id); const d = store.modelDraft(item.id);
        if (!d || d.savedContent() !== item.baseline) { conflicts.push(item.id); continue; } restore(d, item.content);
      } else if (!item.persisted) { const s = state(item.content); store.modelDrafts.set(item.id, new ModelDraft(item.id, s.name, s.rows, store.changed, false)); }
      else conflicts.push(item.id);
    }
    for (const item of r.endpoints) { const existing = store.drafts.get(item.id); if (existing) { if (JSON.stringify(existing) !== JSON.stringify(item.value)) conflicts.push(item.id); continue; } if (JSON.stringify(store.view?.catalog.endpoints.find(e => e.id === item.id)) !== item.baseline) { conflicts.push(item.id); continue; } store.drafts.set(item.id, structuredClone(item.value)); store.draftBaselines.set(item.id, item.baseline); }
    for (const item of r.units) {
      const existing = store.unitDrafts.get(item.unit.id); if (existing?.dirty) { if (existing.content() !== item.content) conflicts.push(item.unit.id); continue; }
      await store.loadTarget('model', item.base.id);
      const u = store.endpoint(item.endpoint)?.units.find(u => u.id === item.unit.id), base = store.view?.catalog.models.find(m => m.id === item.base.id);
      if (JSON.stringify(u) !== JSON.stringify(item.unit) || JSON.stringify(base) !== JSON.stringify(item.base)) { conflicts.push(item.unit.id); continue; }
      const d = new UnitPointDraft(item.endpoint, item.unit, item.base, store.changed); restore(d, item.content); store.unitDrafts.set(item.unit.id, d);
    }
  }
  store.changed(); return [...new Set(conflicts)];
}
