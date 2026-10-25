import { useEffect, useMemo, useState, type ReactNode } from 'react';
import { FolderOpen, FolderPlus, Folder, X, RefreshCw, Minus } from 'lucide-react';
import { bindProjectSession, createProjectClient, listenProjectCommands, type ProjectInfo, type ProjectStatus } from '../../shared/api/project';
import { operateWindow, windowPlatform, armCloseGuard, syncProjectWindow } from '../../shared/api/window';
import artwork from '../../assets/project-entry.png';
/** 工程状态先于工作台装配，空态不创建任何业务 Store 或 Dockview。 */
export function ProjectEntry({ children }: { children: (project: ProjectInfo) => ReactNode }) {
  const client = useMemo(createProjectClient, []), platform = windowPlatform();
  const [status, setStatus] = useState<ProjectStatus | null>(null), [error, setError] = useState(''), [busy, setBusy] = useState(false);
  const refresh = async () => { try { const next = await client.status(); bindProjectSession(next.project?.session ?? ''); setStatus(next); setError(next.startupError ?? ''); } catch (e) { setStatus({ project: null, startupError: null }); setError(message(e)); } };
  useEffect(() => { let active = true; void client.status().then(next => { if (active) { bindProjectSession(next.project?.session ?? ''); setStatus(next); setError(next.startupError ?? ''); } }).catch(e => { if (active) { setStatus({ project: null, startupError: null }); setError(message(e)); } }); return () => { active = false; }; }, [client]);
  const open = async (action: 'open' | 'create', entry?: string) => {
    if (busy) return; setBusy(true); setError('');
    try { const target = await client.prepare(action, entry); if (!target) return; const project = await client.commit(target.token); bindProjectSession(project.session); setStatus({ project, startupError: null }); } catch (e) { setError(message(e)); } finally { setBusy(false); }
  };
  useEffect(() => {
    if (!status || status.project) return;
    let alive = true, cancel: (() => void) | undefined, close: (() => void) | undefined;
    void listenProjectCommands(action => { if (action !== 'close') void open(action); }).then(fn => { if (alive) cancel = fn; else fn(); });
    void armCloseGuard(() => busy, () => {}).then(fn => { if (alive) close = fn; else fn(); });
    const key = (e: KeyboardEvent) => { if (platform !== 'browser') return; if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'o' && !e.repeat) { e.preventDefault(); void open('open'); } };
    window.addEventListener('keydown', key);
    return () => { alive = false; cancel?.(); close?.(); window.removeEventListener('keydown', key); };
  }, [status?.project, !!status, busy]);
  useEffect(() => { if (status) void syncProjectWindow().catch(e => setError(message(e))); }, [!!status, !!status?.project]);
  if (status?.project) return <>{children(status.project)}</>;
  const windowAction = (action: 'drag' | 'minimize' | 'maximize' | 'close') => { void operateWindow(action).catch(e => setError(message(e))); };
  return <main className={`project-entry platform-${platform}`} aria-busy={busy}>
    <header className="entry-titlebar" onPointerDown={e => { if (e.button !== 0 || (e.target as HTMLElement).closest('button')) return; e.preventDefault(); if (e.detail !== 2) windowAction('drag'); }}><span>ESSim</span>{(platform === 'windows' || platform === 'linux') && <div><button aria-label="最小化窗口" onClick={() => windowAction('minimize')}><Minus size={15}/></button><button aria-label="关闭窗口" onClick={() => windowAction('close')}><X size={16}/></button></div>}</header>
    {!status ? <div className="project-checking" role="status">正在检查最近工程…</div> : <div className="entry-layout">
      <aside className="entry-brand"><h1>储能协议工作台</h1><p>从工程开始</p><img src={artwork} alt="工程文件夹、点表与电池模组"/><small>配置与资源，随工程保存。</small></aside>
      <section className="entry-main"><h2>开始</h2><div className="entry-actions"><button className="entry-open" disabled={busy} onClick={() => void open('open')}><FolderOpen/><span><strong>打开工程</strong><small>选择已有工程文件</small></span><kbd>{platform === 'macos' ? '⌘' : 'Ctrl'} O</kbd></button><button disabled={busy} onClick={() => void open('create')}><FolderPlus/><span><strong>新建工程</strong><small>选择空文件夹创建工程</small></span></button></div>
        {error && <p className="entry-error" role="alert">{error}</p>}{platform === 'browser' && <p className="entry-hint">工程文件操作需要桌面工作台，浏览器仅用于界面预览。</p>}
        <div className="entry-recent-heading"><h2>最近工程</h2><button disabled={busy} title="刷新最近工程" aria-label="刷新最近工程" onClick={() => void refresh()}><RefreshCw size={16}/></button></div>
        <div className="entry-recent-list">{status.recent?.length ? status.recent.map(item => <div className="entry-recent" key={item.entry}><button disabled={busy} onClick={() => void open('open', item.entry)} title={item.entry}><Folder size={20}/><span><strong>{item.name}</strong><small>{item.entry}</small></span><time>{item.openedAt ? new Date(item.openedAt).toLocaleDateString() : ''}</time></button><button disabled={busy} aria-label={`移出最近列表：${item.name}`} title="仅移出列表，不删除工程" onClick={() => { setBusy(true); void client.removeRecent(item.entry).then(refresh).catch(e => setError(message(e))).finally(() => setBusy(false)); }}><X size={14}/></button></div>) : <p className="entry-hint">暂无最近工程，打开或新建一个工程开始。</p>}</div>
        <p className="entry-footnote">打开 project.workspace 文件进入工作台。</p>
      </section>
    </div>}
  </main>;
}
function message(e: unknown) { return typeof e === 'object' && e && 'message' in e ? String(e.message) : String(e); }
