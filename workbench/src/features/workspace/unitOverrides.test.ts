import { describe, it, expect, vi } from 'vitest';
import { buildOverrides, effectiveRows, UnitPointDraft, emptyOverrides } from './unitOverrides';
import { WorkspaceStore } from './model';
import type { CollectionModel, UnitConfig, WorkspaceClient, WorkspaceView } from '../../shared/api/contracts';
const base: CollectionModel = { id: 'm', name: '模型', points: [{ id: 'p', name: '电压', category: 'measurement', area: 'holding', address: 0, valueType: 'u16', scale: '1', unit: 'V', initial: { kind: 'unsigned', value: '12' }, initialEngineering: '12', byteOrder: 'big', wordOrder: 'big', note: '', slaveWritable: false, masterPeriodMs: 1000 }] };
const unit: UnitConfig = { id: 'u', name: 'A', station: 1, enabled: true, model: { id: 'm' }, device: null, overrides: { pointOverrides: { p: { note: '独立', address: 10 } }, addedPoints: [], removedPointIds: [] } };
function model(rows: ReturnType<typeof effectiveRows>): CollectionModel { return { ...base, points: rows.map(r => ({ ...base.points[0]!, ...r, address: Number(r.address), slaveWritable: r.slaveWritable === 'true', masterPeriodMs: Number(r.masterPeriodMs), initial: { kind: 'unsigned', value: r.initialEngineering! } } as CollectionModel['points'][number])) }; }
function view(): WorkspaceView { return { sequence: '1', catalog: { models: [structuredClone(base)], endpoints: [{ id: 'e', name: 'E', role: 'slave', transport: { kind: 'tcp', address: '127.0.0.1', port: 1502 }, units: [structuredClone(unit)] }], applied: [], devices: [], values: {} }, capabilities: { configuration: true, execution: true, recording: true, replay: false, sessionOnly: true, reason: '' } }; }
describe('单层 Unit 编辑', () => {
    it('同值显式覆盖保留，恢复字段和撤销都不影响其他覆盖', () => { const d = new UnitPointDraft('e', unit, base, () => { }); d.restore({ fields: { p: ['note'] }, added: [], removed: [] }); expect(d.overrides(model(d.rows)).pointOverrides.p).toEqual({ address: 10 }); d.undo(); expect(d.overrides(model(d.rows)).pointOverrides.p).toEqual({ address: 10, note: '独立' }); });
    it('局部新增和隐藏由身份表达，恢复全部产生可撤销草稿', () => { const d = new UnitPointDraft('e', unit, base, () => { }); d.change(s => { s.rows = [{ ...s.rows[0]!, id: 'local', address: '20' }]; }); const o = d.overrides(model(d.rows)); expect(o.removedPointIds).toEqual(['p']); expect(o.addedPoints[0]?.id).toBe('local'); d.restore(); expect(d.overrides(model(d.rows))).toEqual(emptyOverrides()); d.undo(); expect(d.rows[0]?.id).toBe('local'); });
    it('基础同值保存不会清除已保存覆盖', () => { const b = structuredClone(base); b.points[0]!.note = '独立'; const m = model(effectiveRows(b, unit)); expect(buildOverrides(b, unit.overrides, m).pointOverrides.p?.note).toBe('独立'); });
    it('重继承保留本地编辑，采用最新未编辑基础字段', () => { const s = new WorkspaceStore({} as WorkspaceClient); s.receive(view()); const d = s.unitDraft('e', 'u')!; d.change(x => { x.rows[0]!.name = '自己修改'; }); s.view!.catalog.models[0]!.points[0]!.unit = 'kV'; s.rebaseUnitDraft('e', 'u'); const next = s.unitDraft('e', 'u')!; expect(next.rows[0]!.name).toBe('自己修改'); expect(next.rows[0]!.unit).toBe('kV'); expect(next.rows[0]!.address).toBe('10'); expect(next.dirty).toBe(true); });
    it('失效基础点不被重解释为局部新增，草稿保留', () => { const s = new WorkspaceStore({} as WorkspaceClient); s.receive(view()); const d = s.unitDraft('e', 'u')!; d.change(x => { x.rows[0]!.name = '编辑'; }); s.view!.catalog.models[0]!.points = []; expect(() => s.rebaseUnitDraft('e', 'u')).toThrow('基础点'); expect(s.unitDrafts.get('u')).toBe(d); });
    it('整端点保存包含编辑器差异，校验失败不提交', async () => { const v = view(); const execute = vi.fn(async (c) => { v.catalog.endpoints = [c.action.endpoint]; v.sequence = '2'; return { objectId: 'e', sequence: '2' }; }); const client = { subscribe: async () => async () => { }, query: async () => structuredClone(v), execute, previewPoints: async (input) => ({ model: model(input.points), errors: [], previews: [] }) } as WorkspaceClient; const s = new WorkspaceStore(client); s.receive(structuredClone(v)); s.unitDraft('e', 'u')!.change(x => { x.rows[0]!.address = '25'; }); await s.saveEndpointDraft('e'); expect(execute.mock.calls[0]![0].action.endpoint.units[0].overrides.pointOverrides.p.address).toBe(25); expect(s.endpointDirty('e')).toBe(false); s.unitDraft('e', 'u')!.change(x => { x.rows[0]!.name = ''; }); client.previewPoints = async () => ({ model: null, errors: [{ code: 'INVALID', message: '点名不能为空' }], previews: [] }); await expect(s.saveEndpointDraft('e')).rejects.toMatchObject({ code: 'INVALID' }); expect(execute).toHaveBeenCalledTimes(1); expect(s.endpointDirty('e')).toBe(true); });
    it('基础改变时禁止用旧整表保存，保留用户输入', async () => { const execute = vi.fn(); const s = new WorkspaceStore({ execute } as unknown as WorkspaceClient); s.receive(view()); s.unitDraft('e', 'u')!.change(x => { x.rows[0]!.note = '改动'; }); s.view!.catalog.models[0]!.points[0]!.unit = 'kV'; await expect(s.saveEndpointDraft('e')).rejects.toThrow('基础模型已改变'); expect(execute).not.toHaveBeenCalled(); expect(s.unitDrafts.get('u')!.rows[0]!.note).toBe('改动'); });
});
it('异步校验仅保存提交时差异，期间恢复字段仍保留为未保存草稿', async () => {
    const v = view();
    let release!: (value: any) => void;
    const client = { subscribe: async () => async () => { }, query: async () => structuredClone(v), execute: vi.fn(async (c) => { v.catalog.endpoints = [c.action.endpoint]; v.sequence = '2'; return { objectId: 'e', sequence: '2' }; }), previewPoints: vi.fn(input => new Promise(resolve => { release = () => resolve({ model: model(input.points), errors: [], previews: [] }); })) } as WorkspaceClient;
    const s = new WorkspaceStore(client);
    s.receive(structuredClone(v));
    const d = s.unitDraft('e', 'u')!;
    d.change(x => { x.rows[0]!.name = '提交名称'; });
    const saving = s.saveEndpointDraft('e');
    await Promise.resolve();
    d.restore({ fields: { p: ['note'] }, added: [], removed: [] });
    release(undefined);
    await saving;
    expect(v.catalog.endpoints[0]!.units[0]!.overrides!.pointOverrides.p?.note).toBe('独立');
    expect(d.rows[0]!.note).toBe('');
    expect(d.dirty).toBe(true);
    expect(s.unitDrafts.get('u')).toBe(d);
});
it('恢复隐藏基础点时维持模型顺序，局部点保留在末尾', () => { const b = structuredClone(base); b.points.push({ ...b.points[0]!, id: 'second', address: 1 }); const d = new UnitPointDraft('e', unit, b, () => { }); d.change(s => { s.rows.shift(); s.rows.push({ ...s.rows[0]!, id: 'local', address: '40' }); }); d.restore({ fields: {}, added: [], removed: ['p'] }); expect(d.rows.map(p => p.id)).toEqual(['p', 'second', 'local']); });
