import { useEffect, useRef, useState } from 'react';
import { EditorBody, type EditorToolbar } from './PointEditor';
import { WorkspaceStore, errorMessage } from './model';
import { allOverrideSelection, overrideCount, normalizedOverrides } from './unitOverrides';
import { labels, fields, modelRows } from './pointDraft';
import type { OverrideField, OverrideSelection, PromotionPlan, InitialSample } from '../../shared/api/contracts';
export function UnitPointEditor({ store, endpoint, unit, toolbar }: {
    store: WorkspaceStore;
    endpoint: string;
    unit: string;
    toolbar?: EditorToolbar;
}) {
    const draft = store.unitDraft(endpoint, unit);
    const [dialog, setDialog] = useState<'promotion' | 'extract' | null>(null);
    const [error, setError] = useState('');
    const [showOverrides,setShowOverrides] = useState(false);
    if (!draft)
        return <div className="empty">正在读取有效点表…</div>;
    const saved = store.view?.catalog.endpoints.find(e => e.id === endpoint)?.units.find(u => u.id === unit);
    const o = normalizedOverrides(saved?.overrides);
    const changed = draft.changedBase(store.view?.catalog.models.find(m => m.id === draft.base.id));
    const visibleIds = new Set(draft.rows.map(p => p.id));
    const hidden = draft.base.points.filter(p => !visibleIds.has(p.id)).map(p => p.id);
    const beforeDialog = async (kind: 'promotion' | 'extract') => { try {
        setError('');
        if (store.modelDrafts.get(draft.base.id)?.dirty)
            throw Error('引用模型有未保存草稿，请先在模型编辑页保存或放弃。');
        if (store.endpointDirty(endpoint))
            await store.saveEndpointDraft(endpoint);
        setDialog(kind);
    }
    catch (e) {
        setError(errorMessage(e));
    } };
    const selected = draft.rows.find(p => p.id === draft.selected);
    const parentPoint = draft.base.points.find(p => p.id === selected?.id);
    const parent = parentPoint ? modelRows({ ...draft.base, points: [parentPoint] })[0] : undefined;
    const patch = selected && parent ? Object.fromEntries(fields.filter(f => selected[f] !== parent[f] || (Object.hasOwn(o.pointOverrides[selected.id!] ?? {}, f) && !draft.inheritedFields.includes(`${selected.id}:${f}`))).map(f => [f, selected[f]])) : undefined;
    return <>
  <EditorBody key={`${unit}:${draft.base.id}`} store={store} draft={draft} owner={{ endpoint, unit }} toolbar={{...toolbar,
    info:<> · {overrideCount(saved?.overrides)} 差异</>,
    menu:[...(toolbar?.menu??[]),
      {label:"保存并预览回写",disabled:store.busy,action:()=>void beforeDialog('promotion')},
      {label:"恢复全部模型配置",disabled:store.busy,action:()=>{store.flushUnitEditors.get(unit)?.();draft.restore();}},
      {label:"另存为新模型并绑定",disabled:store.busy,action:()=>void beforeDialog('extract')},
      {label:"查看继承、覆盖与隐藏点",action:()=>setShowOverrides(v=>!v)},
    ]}} extra={<>
    {showOverrides&&<details className="override-details" open><summary>继承、覆盖与隐藏点</summary>
      <div className="panel-note">基础：{draft.base.name} · {overrideCount(saved?.overrides)} 项已保存差异</div>
      {selected&&<div className="panel-note">选中点：{selected.name} · {!parent?'局部新增':Object.keys(patch??{}).length?`已覆盖：${Object.keys(patch??{}).map(f=>labels[f]??f).join('、')}`:'继承模型'}</div>}
      {!!hidden.length&&<div className="panel-note">隐藏点：{hidden.map(id=><button key={id} onClick={()=>draft.restore({fields:{},added:[],removed:[id]})}>恢复 {draft.base.points.find(p=>p.id===id)?.name??id}</button>)}</div>}
      {selected&&<div className="override-fields">{fields.map(f=><span key={f}>{labels[f]} · {!parent?'局部新增':Object.hasOwn(patch??{},f)?'已覆盖':'继承'} <button disabled={!parent} onClick={()=>draft.restore({fields:{[selected.id!]:[f]},added:[],removed:[]})}>恢复</button></span>)}</div>}
    </details>}
    {changed && <div className="configuration-notice error" role="alert">基础模型已改变，当前草稿已保留。<button onClick={() => { try {
            store.rebaseUnitDraft(endpoint, unit);
            setError('');
        }
        catch (e) {
            setError(errorMessage(e));
        } }}>保留编辑并重新继承</button><button onClick={() => { store.unitDrafts.delete(unit); store.changed(); }}>放弃点表草稿</button></div>}
    {error && <div className="error panel-note" role="alert">{error}</div>}
  </>}/>
  {dialog && <OverrideDialog store={store} endpoint={endpoint} unit={unit} mode={dialog} close={() => setDialog(null)}/>}
 </>;
}
function OverrideDialog({ store, endpoint, unit, mode, close }: {
    store: WorkspaceStore;
    endpoint: string;
    unit: string;
    mode: 'promotion' | 'extract';
    close: () => void;
}) {
    const ref = useRef<HTMLDialogElement>(null);
    const saved = store.view!.catalog.endpoints.find(e => e.id === endpoint)!.units.find(u => u.id === unit)!;
    const [selection, setSelection] = useState<OverrideSelection>(() => allOverrideSelection(saved.overrides));
    const [plan, setPlan] = useState<PromotionPlan | null>(null);
    const [error, setError] = useState('');
    const [busy, setBusy] = useState(false);
    const [name, setName] = useState(`${saved.name} 模型`);
    useEffect(() => { ref.current?.showModal(); }, []);
    const load = async () => { setBusy(true); setError(''); setPlan(null); try {
        const p = await store.client.previewUnitPromotion?.(endpoint, unit, selection);
        if (!p)
            throw Error('回写预览服务未接入');
        setPlan(p);
    }
    catch (e) {
        setError(errorMessage(e));
    }
    finally {
        setBusy(false);
    } };
    const toggle = (id: string, field: OverrideField | 'added' | 'removed', checked: boolean) => { setPlan(null); setSelection(s => { const next = structuredClone(s); if (field === 'added' || field === 'removed')
        next[field] = checked ? [...next[field], id] : next[field].filter(v => v !== id);
    else {
        const fs = next.fields[id] ?? [];
        next.fields[id] = checked ? [...fs, field] : fs.filter(f => f !== field);
    } return next; }); };
    const all = allOverrideSelection(saved.overrides);
    const submit = async () => { setBusy(true); setError(''); try {
        if (store.endpointDirty(endpoint))
            throw Error('实例又有未保存修改，请关闭后重新保存并预览');
        if (store.modelDrafts.get(saved.model.id)?.dirty)
            throw Error('模型有未保存草稿，请先处理');
        if (mode === 'extract') {
            await store.execute({ kind: 'extractUnitModel', endpoint, unit, id: crypto.randomUUID(), name });
        }
        else {
            if (!plan)
                throw Error('请先预览');
            await store.execute({ kind: 'promoteOverrides', plan });
        }
        store.unitDrafts.delete(unit);
        const md = store.modelDrafts.get(saved.model.id);
        if (md && !md.dirty)
            store.modelDrafts.delete(saved.model.id);
        store.changed();
        close();
    }
    catch (e) {
        setError(errorMessage(e));
        setPlan(null);
    }
    finally {
        setBusy(false);
    } };
    return <dialog ref={ref} aria-label={mode === 'extract' ? '另存为新模型并绑定' : '回写模型'} onCancel={e => { e.preventDefault(); if (!busy)
        close(); }} className="override-dialog"><h2>{mode === 'extract' ? '另存为新模型并绑定' : '回写模型'}</h2>
 {mode === 'extract' ? <><label>模型名称<input value={name} onChange={e => setName(e.target.value)}/></label><p>生成独立模型与点身份，清空当前 Unit 覆盖。原运行快照保留；下次应用会按新模型初始化，不自动迁移旧值。</p></> : <><p>只回写所选配置差异。保存后不会自动应用或启动任何模拟器。</p><div className="override-selection">{Object.entries(all.fields).flatMap(([id, fs]) => fs.map(f => <label key={`${id}:${f}`}><input type="checkbox" checked={selection.fields[id]?.includes(f) ?? false} onChange={e => toggle(id, f, e.target.checked)}/>{store.view?.catalog.models.find(m => m.id === saved.model.id)?.points.find(p => p.id === id)?.name ?? id} · {labels[f]}</label>))}{(['added', 'removed'] as const).flatMap(kind => all[kind].map(id => <label key={`${kind}:${id}`}><input type="checkbox" checked={selection[kind].includes(id)} onChange={e => toggle(id, kind, e.target.checked)}/>{kind === 'added' ? '新增基础点' : '删除基础点'} · {(kind === 'added' ? saved.overrides?.addedPoints : store.view?.catalog.models.find(m => m.id === saved.model.id)?.points)?.find(p => p.id === id)?.name ?? id}</label>))}</div>
 {plan && <><p>目标：{plan.before.name} · {plan.references.length} 个引用 Unit · {new Set(plan.references.map(r => r.endpoint)).size} 个端点</p><div className="override-diff">{Object.entries(plan.selection.fields).flatMap(([id, fs]) => fs.map(f => <div key={`${id}:${f}`}>{plan.before.points.find(p => p.id === id)?.name} · {labels[f]}：{String(plan.before.points.find(p => p.id === id)?.[f] ?? '')} → {String(plan.after.points.find(p => p.id === id)?.[f] ?? '')}</div>))}{plan.references.map(r => <div key={r.unit.id}>{store.view?.catalog.endpoints.find(e => e.id === r.endpoint)?.name} / {r.unit.name} · {store.view?.applicationStates?.[r.endpoint] ?? '未运行'} · 协议变更待显式应用</div>)}</div></>}
 </>}{error && <p role="alert" className="error">{error}</p>}<div className="dialog-actions"><button onClick={close} disabled={busy}>取消</button>{mode === 'promotion' && <button onClick={() => void load()} disabled={busy}>刷新预览</button>}<button disabled={busy || (mode === 'promotion' ? !plan : !name.trim())} onClick={() => void submit()}>{mode === 'extract' ? '另存并绑定' : '确认回写'}</button></div></dialog>;
}
export function CaptureInitialsDialog({ store, endpoint, unit, points, close }: {
    store: WorkspaceStore;
    endpoint: string;
    unit: string;
    points: string[];
    close: () => void;
}) {
    const baseline = useRef('');
    const baselineContent = () => JSON.stringify({ unit: store.view?.catalog.endpoints.find(e => e.id === endpoint)?.units.find(u => u.id === unit), base: store.view?.catalog.models.find(m => m.id === store.view?.catalog.endpoints.find(e => e.id === endpoint)?.units.find(u => u.id === unit)?.model.id) });
    const ref = useRef<HTMLDialogElement>(null);
    const [samples, setSamples] = useState<InitialSample[] | null>(null);
    const [error, setError] = useState('');
    useEffect(() => { ref.current?.showModal(); let active = true; baseline.current = baselineContent(); void store.client.captureInitials?.(endpoint, unit, points).then(s => { if (active)
        setSamples(s); }).catch(e => { if (active)
        setError(errorMessage(e)); }); return () => { active = false; }; }, [store, endpoint, unit]);
    return <dialog ref={ref} aria-label="捕获默认初始值" onCancel={e => { e.preventDefault(); close(); }}><h2>将确认值设为默认初始值</h2><p>以下为固定取样，只生成配置草稿，不保存、不回写、不下发。</p>{samples?.map(s => <div key={s.point}>{store.unitDraft(endpoint, unit)?.rows.find(p => p.id === s.point)?.name ?? s.point}：{s.engineering} · {new Date(Number(s.unixMs)).toLocaleString()}</div>)}{error && <p role="alert" className="error">{error}</p>}<div className="dialog-actions"><button onClick={close}>取消</button><button disabled={!samples} onClick={async () => { try {
        await store.refresh();
        await store.loadTarget('endpoint', endpoint, true);
        if (baseline.current !== baselineContent())
            throw Error('配置已改变，请取消后重新取样');
        const draft = store.unitDraft(endpoint, unit);
        if (!draft)
            throw Error('点表未加载');
        if (draft.dirty)
            throw Error('请先保存或放弃配置草稿后再捕获');
        draft.change(state => { for (const s of samples!) {
            const row = state.rows.find(p => p.id === s.point);
            if (!row)
                throw Error('点已改变，请重新取样');
            row.initialEngineering = s.engineering;
        } });
        store.endpointModes.set(endpoint, 'configuration');
        store.changed();
        close();
    }
    catch (e) {
        setError(errorMessage(e));
    } }}>生成初值草稿</button></div></dialog>;
}
