import { isTauri } from '@tauri-apps/api/core';
import { createTauriClient } from '../shared/api/tauri';
import type { EssimClient } from '../shared/api/client';
export function createClient(): EssimClient {
  if (isTauri()) return createTauriClient();
  const unavailable = async (): Promise<never> => { throw { code: 'BACKEND_UNAVAILABLE', message: '浏览器预览未连接 Rust 核心，请通过 Tauri 启动。' }; };
  return { getBackendInfo: unavailable, getSnapshot: unavailable, start: unavailable, configure: unavailable, startRtu: unavailable, stop: unavailable, updateSoc: unavailable, addUnit: unavailable, subscribe: unavailable };
}
