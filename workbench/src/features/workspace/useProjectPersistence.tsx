import { useEffect, useMemo, useRef, useState, type RefObject } from 'react';
import type { DockviewApi, SerializedDockview } from 'dockview-react';
import { WorkbenchDialog, DialogBody, DialogFooter } from '@essim/ui/components/workbench-dialog';
import { createProjectClient, listenProjectCommands, listenRecentProjects, type ProjectPreparation } from '../../shared/api/project';
import { draftRecovery, hasRecovery, restoreDraftRecovery, combineRecovery, recoverySets, type DraftRecovery } from './projectRecovery';
import { errorMessage, type WorkspaceStore, type Target } from './model';

/** 工程切换成功后重建前端会话，避免旧草稿与异步加载污染新工程。 */
export function useProjectPersistence(store: WorkspaceStore, api: RefObject<DockviewApi | null>, select: (target: Target) => void) {
  const client = useMemo(createProjectClient, []);
  const [action, setAction] = useState<'create' | 'open' | 'close' | null>(null);
  const [busy, setBusy] = useState(false);
  const prepared = useRef<ProjectPreparation | null>(null);
  const [recent, setRecent] = useState<import('../../shared/api/project').RecentProject[]>([]);
  store.projectBusy = busy;
  const [recovery, setRecovery] = useState<DraftRecovery | null>(null);
  const initialized = useRef(false), localReady = useRef(false), preserveRecovery = useRef(false);
  const heldRecovery = useRef<DraftRecovery | null>(null);
  const recoveryPayload = () => combineRecovery(draftRecovery(store), heldRecovery.current);
  const pending = useRef<Promise<void>>(Promise.resolve());
  const fail = (e: unknown) => { store.error = errorMessage(e); store.changed(); };
  const write = (kind: 'workspace' | 'recovery', value: unknown) => {
    const id = store.project ? (store.project.session ?? store.project.id) : null; if (!id) return Promise.resolve();
    const next = pending.current.catch(() => { }).then(() => client.write(id, kind, value)); pending.current = next; return next;
  };
  const flush = async () => {
    if (!store.project) return;
    if (api.current) await write('workspace', api.current.toJSON());
    if (preserveRecovery.current && hasRecovery(draftRecovery(store))) throw Error('恢复区不可读，请先保存当前草稿再退出。');
    if (!preserveRecovery.current) await write('recovery', recoveryPayload());
    await pending.current;
  };
  useEffect(() => {
    if (initialized.current || !store.view || !api.current) return;
    initialized.current = true; let disposed = false; let timer: ReturnType<typeof setTimeout> | undefined; let disposeLayout: (() => void) | undefined;
    const start = async () => {
      const status = await client.status(); if (disposed) return; store.project = status.project; setRecent(status.recent ?? []); if (status.startupError) store.error = status.startupError; store.changed();
      if (!status.project) { localReady.current = true; return; }
      const [layout, drafts] = await Promise.all([client.read(status.project.session ?? status.project.id, 'workspace').catch(e => { fail(e); return null; }), client.read(status.project.session ?? status.project.id, 'recovery').catch(e => { preserveRecovery.current = true; fail(e); return null; })]); if (disposed) return;
      const dock = api.current!; const fallback = dock.toJSON();
      if (layout) {
        try {
          const data = layout as SerializedDockview; const allowed = new Set(['welcome', 'project', 'inspector', 'observations', 'problems', 'model', 'endpoint', 'resource', 'resources', 'batch']);
          if (!data.panels || Object.values(data.panels).some(p => !p.contentComponent || !allowed.has(p.contentComponent))) throw Error('布局组件无效');
          for (const required of ['welcome', 'project', 'inspector-point', 'inspector-unit', 'inspector-endpoint', 'messages', 'observations', 'problems']) if (!data.panels[required]) throw Error('布局缺少固定面板');
          dock.fromJSON(data);
          for (const p of [...dock.panels]) {
            const kind = p.id.startsWith('model:') ? 'model' : p.id.startsWith('endpoint:') ? 'endpoint' : null;
            if (kind) { const id = p.id.slice(p.id.indexOf(':') + 1); const valid = kind === 'model' ? store.view?.catalog.models.some(m => m.id === id) : store.view?.catalog.endpoints.some(e => e.id === id); if (!valid) dock.removePanel(p); else await store.loadTarget(kind, id); }
          }
          const project = dock.getPanel('project'); if (project) project.group.model.header.hidden = true;
          const isDocument = (p: { id: string }) => p.id.startsWith('model:') || p.id.startsWith('endpoint:');
          const active = dock.activePanel && isDocument(dock.activePanel) ? dock.activePanel : dock.panels.find(p => isDocument(p) && p.api.isActive && p.group.id === data.activeGroup) ?? dock.panels.find(p => isDocument(p) && p.api.isActive);
          if (active?.id.startsWith('model:')) select({ kind: 'model', id: active.id.slice(6) });
          else if (active?.id.startsWith('endpoint:')) select({ kind: 'endpoint', id: active.id.slice(9) });
        } catch { dock.fromJSON(fallback); store.error = '上次布局无法恢复，已使用默认布局'; store.changed(); }
      }
      if (drafts !== null) { try { recoverySets(drafts); if (hasRecovery(drafts)) { heldRecovery.current = drafts; setRecovery(drafts); } } catch (e) { preserveRecovery.current = true; fail(e); } }
      localReady.current = true;
      const listener = dock.onDidLayoutChange(() => { clearTimeout(timer); timer = setTimeout(() => { void write('workspace', dock.toJSON()).catch(fail); }, 600); }); disposeLayout = () => listener.dispose();
    };
    void start().catch(fail);
    return () => { disposed = true; initialized.current = false; clearTimeout(timer); disposeLayout?.(); };
  }, [store.view !== null, api.current]);
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const cancel = store.subscribe(() => { if (!localReady.current || preserveRecovery.current || !store.project) return; clearTimeout(timer); timer = setTimeout(() => { void write('recovery', recoveryPayload()).catch(fail); }, 800); });
    return () => { cancel(); clearTimeout(timer); };
  }, [store]);
  useEffect(() => { let alive = true, dispose: (() => void) | undefined; void listenProjectCommands(action => { if (!busy) void request(action); }).then(cancel => { if (alive) dispose = cancel; else cancel(); }); return () => { alive = false; dispose?.(); }; }, [busy]);
  useEffect(() => { let active=true; let dispose:(()=>void)|undefined; void listenRecentProjects(()=>{void client.status().then(s=>{if(active)setRecent(s.recent??[]);}).catch(fail);}).then(fn=>{if(active)dispose=fn;else fn();});return()=>{active=false;dispose?.();}; }, [client]);
  useEffect(() => {
    let active = true, pending = false, last = '';
    const timer = setInterval(() => { if (pending || !store.project) return; pending = true; void client.status().then(status => { const message = status.runtimeData?.error; if (active && message && message !== last) { last = message; fail(`运行数据库记录不完整：${message}`); } }).catch(() => { }).finally(() => { pending = false; }); }, 2000);
    return () => { active = false; clearInterval(timer); };
  }, [client, store]);
  const complete = async (action: 'create' | 'open' | 'close', target: ProjectPreparation | null) => {
    await flush();
    if (action === 'close') await client.close();
    else if (target) await client.commit(target.token);
    window.location.reload();
  };
  const cancel = () => { if (busy) return; if (prepared.current) void client.cancel(prepared.current.token).catch(fail); prepared.current = null; setAction(null); };
  const request = async (next: 'create' | 'open' | 'open-current' | 'close', entry?: string) => {
    if (busy || store.busy) return; setBusy(true);
    try {
      const target = next === 'close' ? null : await client.prepare(next === 'open-current' ? 'open' : next, entry, next === 'open-current');
      if (next !== 'close' && (!target || target.handled)) return;
      if (target?.same) { await client.cancel(target.token); return; }
      prepared.current = target;
      const current = await client.status();
      const needsProtection = hasRecovery(draftRecovery(store)) || current.requiresStop;
      if (needsProtection) setAction(next === 'open-current' ? 'open' : next);
      else await complete(next === 'open-current' ? 'open' : next, target);
    } catch (e) { fail(e); if (prepared.current) void client.cancel(prepared.current.token).catch(fail); prepared.current=null; } finally { setBusy(false); }
  };
  const perform = async () => {
    if (!action || busy) return; setBusy(true);
    try { await complete(action, prepared.current); } catch (e) { fail(e); if(prepared.current) void client.cancel(prepared.current.token).catch(fail); setAction(null); prepared.current = null; } finally { setBusy(false); }
  };
  const decideRecovery = async (restore: boolean) => {
    if (!recovery || busy) return; setBusy(true);
    try {
      if (restore) { const conflicts = await restoreDraftRecovery(store, recovery); if (conflicts.length) { store.error = `${conflicts.length} 项草稿与已保存内容冲突，恢复副本仍保留。`; store.changed(); heldRecovery.current = recovery; await write("recovery", recoveryPayload()); setRecovery(null); return; } }
      heldRecovery.current = null; preserveRecovery.current = false; await write('recovery', draftRecovery(store)); setRecovery(null);
    } catch (e) { fail(e); } finally { setBusy(false); }
  };
  const dialogs = <>
    {action && <WorkbenchDialog title={action === 'create' ? '新建工程' : action === 'open' ? '打开工程' : '关闭工程'} initialWidth={400} busy={busy} onCancel={cancel}><DialogBody><p>{action === 'create' ? '将在所选目录创建空工程，不复制当前资源。' : '切换前会停止当前工程通信。'}</p><p>未保存草稿会保留在当前工程恢复区；运行通信会停止。</p></DialogBody><DialogFooter><button type="button" disabled={busy} onClick={cancel}>取消</button><button type="button" disabled={busy || store.busy} onClick={() => void perform()}>{action === 'close' ? '关闭工程' : '切换工程'}</button></DialogFooter></WorkbenchDialog>}
    {recovery && <WorkbenchDialog title="发现未保存草稿" initialWidth={400} busy={busy} onCancel={() => { if (!busy) setRecovery(null); }}><DialogBody><p>可恢复上次未保存修改。正式资源已变化的草稿会保留副本，不覆盖文件。</p></DialogBody><DialogFooter><button type="button" disabled={busy} onClick={() => void decideRecovery(false)}>丢弃草稿</button><button type="button" disabled={busy} onClick={() => void decideRecovery(true)}>恢复草稿</button></DialogFooter></WorkbenchDialog>}
  </>;
  return { request: (action: 'create' | 'open' | 'open-current' | 'close', entry?: string) => { void request(action, entry); }, recent, dialogs, flush };
}
