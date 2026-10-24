import { expect, it } from 'vitest';
import { WorkspaceStore } from './model';
import { ModelDraft, defaultRow } from './pointDraft';
import { draftRecovery, restoreDraftRecovery, hasRecovery } from './projectRecovery';
import type { WorkspaceClient, WorkspaceView } from '../../shared/api/contracts';
function store() { return new WorkspaceStore({} as WorkspaceClient); }
it('未保存的新模型恢复后仍是脏草稿，不伪装成已保存', async () => {
  const original = store(); const d = new ModelDraft('new', '未保存模型', [defaultRow('measurement', 'p')], undefined, false); original.modelDrafts.set('new', d);
  const recovery = draftRecovery(original); expect(hasRecovery(recovery)).toBe(true); const target = store(); expect(await restoreDraftRecovery(target, recovery)).toEqual([]); expect(target.modelDrafts.get('new')?.dirty).toBe(true); expect(target.modelDrafts.get('new')?.rows[0]?.id).toBe('p');
});
it('已删除的已保存模型不被草稿自动重建', async () => {
  const original = store(); const d = new ModelDraft('m', '模型', []); d.change(v => { v.name = '修改'; }); original.modelDrafts.set('m', d); const target = store(); expect(await restoreDraftRecovery(target, draftRecovery(original))).toEqual(['m']); expect(target.modelDrafts.size).toBe(0);
});
it('正式模型被外部修改后保留冲突，不覆盖新内容', async () => {
  const original = store(); const d = new ModelDraft('m', '旧名称', []); d.change(v => { v.name = '草稿'; }); original.modelDrafts.set('m', d);
  const target = store(); target.view = { catalog: { models: [{ id: 'm', name: '新名称', points: [] }], endpoints: [], devices: [], applied: [], values: {} } } as unknown as WorkspaceView;
  expect(await restoreDraftRecovery(target, draftRecovery(original))).toEqual(['m']); expect(target.modelDrafts.get('m')?.name).toBe('新名称'); expect(target.modelDrafts.get('m')?.dirty).toBe(false);
});
it('一致的正式模型可以恢复草稿，保存基线保持不变', async () => {
  const original = store(); const d = new ModelDraft('m', '旧名称', []); d.change(v => { v.name = '草稿'; }); original.modelDrafts.set('m', d);
  const target = store(); target.view = { catalog: { models: [{ id: 'm', name: '旧名称', points: [] }], endpoints: [], devices: [], applied: [], values: {} } } as unknown as WorkspaceView;
  expect(await restoreDraftRecovery(target, draftRecovery(original))).toEqual([]); expect(target.modelDrafts.get('m')?.name).toBe('草稿'); expect(target.modelDrafts.get('m')?.dirty).toBe(true);
});
it('暂不恢复后产生的新草稿与旧草稿都保留，恢复时不覆盖较新的输入', async () => {
  const { combineRecovery, recoverySets } = await import('./projectRecovery');
  const old = store(); old.modelDrafts.set('new', new ModelDraft('new', '旧草稿', [], undefined, false));
  const newer = store(); newer.modelDrafts.set('new', new ModelDraft('new', '新草稿', [], undefined, false));
  const combined = combineRecovery(draftRecovery(newer), draftRecovery(old)); expect(recoverySets(combined)).toHaveLength(2);
  const target = store(); expect(await restoreDraftRecovery(target, combined)).toEqual(['new']); expect(target.modelDrafts.get('new')?.name).toBe('新草稿'); expect(combined.deferred?.[0]?.models[0]?.content).toContain('旧草稿');
});
it('工程保存失败时保留脏标记与原保存基线', async () => {
  const original = store(); original.view = { sequence: '1', catalog: { models: [{ id: 'm', name: '正式', points: [] }], endpoints: [], devices: [], applied: [], values: {} } } as unknown as WorkspaceView;
  original.client.previewPoints = async () => ({ model: { id: 'm', name: '草稿', points: [] }, errors: [], warnings: [], previews: [] }); original.client.execute = async () => { throw { code: 'PROJECT_IO', message: '磁盘已满' }; };
  const d = original.modelDraft('m')!; const baseline = d.savedContent(); d.change(s => { s.name = '草稿'; }); await expect(original.saveModelDraft('m')).rejects.toMatchObject({ code: 'PROJECT_IO' }); expect(d.dirty).toBe(true); expect(d.savedContent()).toBe(baseline); expect(d.name).toBe('草稿');
});
