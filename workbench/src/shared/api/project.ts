/** 工程管理的桌面适配边界，业务组件不直接访问 Tauri。 */
import { listen } from '@tauri-apps/api/event';
import { listenWindowEvent } from './window';
import { invoke, isTauri } from '@tauri-apps/api/core';
export interface ProjectDocumentInfo { id: string; name: string }
export interface ProjectInfo { documents: ProjectDocumentInfo[]; session: string; id: string; name: string; entry: string }
export interface RecentProject { entry: string; name: string; openedAt: number }
export interface ProjectPreparation { token: string; name: string; same: boolean; handled?: boolean }
let activeSession = "";
export function bindProjectSession(session: string) { activeSession = session; }
export function projectSession() { return activeSession; }
export interface ProjectStatus { requiresStop?: boolean; recent?: RecentProject[]; project: ProjectInfo | null; startupError: string | null; runtimeData?: { queuedBytes: number; failedRuns: number; error: string | null } | null }
export interface ProjectClient {
  status(): Promise<ProjectStatus>;
  loadDocument(session: string, id: string): Promise<string>;
  prepare(action: 'create' | 'open', entry?: string, currentWindow?: boolean): Promise<ProjectPreparation | null>;
  commit(token: string): Promise<ProjectInfo>;
  cancel(token: string): Promise<void>;
  removeRecent(entry: string): Promise<void>;
  close(): Promise<void>;
  read(id: string, kind: 'workspace' | 'recovery'): Promise<unknown>;
  write(id: string, kind: 'workspace' | 'recovery', value: unknown): Promise<void>;
}
export function createProjectClient(): ProjectClient {
  const unavailable = async (): Promise<never> => { throw Error('工程文件操作需要桌面工作台'); };
  if (!isTauri()) return { status: async () => ({ project: null, startupError: null }), prepare: unavailable, commit: unavailable, cancel: unavailable, removeRecent: unavailable, loadDocument: unavailable, close: unavailable, read: unavailable, write: unavailable };
  return { status: () => invoke('query_project'), loadDocument: (session, id) => invoke('load_project_document', { session, id }), prepare: (action, entry, currentWindow) => invoke('prepare_project', { action, entry, currentWindow }), commit: token => invoke('commit_project', { token }), cancel: token => invoke('cancel_prepared_project', { token }), removeRecent: entry => invoke('remove_recent_project', { entry }), close: () => invoke('close_project'), read: (id, kind) => invoke('read_project_local', { id, kind }), write: (id, kind, value) => invoke('write_project_local', { id, kind, value }) };
}

export async function listenProjectCommands(handle: (action: 'create' | 'open' | 'open-current' | 'close') => void): Promise<() => void> {
  if (!isTauri()) return () => { };
  return listenWindowEvent<'create' | 'open' | 'open-current' | 'close'>('workbench-project', event => handle(event.payload));
}

/** 最近列表由进程共享，通知仅刷新列表，不重建工程状态。 */
export async function listenRecentProjects(handle: () => void): Promise<() => void> {
  return isTauri() ? listen('recent-changed', handle) : () => {};
}
