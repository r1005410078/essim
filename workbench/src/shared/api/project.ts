/** 工程管理的桌面适配边界，业务组件不直接访问 Tauri。 */
import { listen } from '@tauri-apps/api/event';
import { invoke, isTauri } from '@tauri-apps/api/core';
export interface ProjectDocumentInfo { id: string; name: string }
export interface ProjectInfo { documents: ProjectDocumentInfo[]; session: string; id: string; name: string; entry: string }
export interface ProjectStatus { project: ProjectInfo | null; startupError: string | null; runtimeData?: { queuedBytes: number; failedRuns: number; error: string | null } | null }
export interface ProjectClient {
  status(): Promise<ProjectStatus>;
  loadDocument(session: string, id: string): Promise<string>;
  choose(action: 'create' | 'open'): Promise<ProjectInfo | null>;
  close(): Promise<void>;
  read(id: string, kind: 'workspace' | 'recovery'): Promise<unknown>;
  write(id: string, kind: 'workspace' | 'recovery', value: unknown): Promise<void>;
}
export function createProjectClient(): ProjectClient {
  const unavailable = async (): Promise<never> => { throw Error('工程文件操作需要桌面工作台'); };
  if (!isTauri()) return { status: async () => ({ project: null, startupError: null }), choose: unavailable, loadDocument: unavailable, close: unavailable, read: unavailable, write: unavailable };
  return { status: () => invoke('query_project'), loadDocument: (session, id) => invoke('load_project_document', { session, id }), choose: action => invoke('choose_project', { action }), close: () => invoke('close_project'), read: (id, kind) => invoke('read_project_local', { id, kind }), write: (id, kind, value) => invoke('write_project_local', { id, kind, value }) };
}

export async function listenProjectCommands(handle: (action: 'create' | 'open' | 'close') => void): Promise<() => void> {
  if (!isTauri()) return () => { };
  return listen<'create' | 'open' | 'close'>('workbench-project', event => handle(event.payload));
}
