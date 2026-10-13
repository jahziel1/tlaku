import type { LaserApi } from './api';
import { createMockApi } from './mock';
import { createTauriApi } from './tauri';

export type { LaserApi, Unlisten } from './api';
export * from './types';

export type ApiKind = 'mock' | 'tauri';

/** VITE_API=mock|tauri. Por defecto, mock. */
export function resolveApiKind(raw: string | undefined = import.meta.env.VITE_API): ApiKind {
  return raw === 'tauri' ? 'tauri' : 'mock';
}

let current: LaserApi | null = null;
let currentKind: ApiKind | null = null;

/** Implementación activa (singleton). Se crea la primera vez que se pide. */
export function getApi(): LaserApi {
  if (!current) {
    currentKind = resolveApiKind();
    current = currentKind === 'tauri' ? createTauriApi() : createMockApi();
  }
  return current;
}

export function getApiKind(): ApiKind {
  getApi();
  return currentKind ?? 'mock';
}

/** Para pruebas: sustituye la implementación activa. */
export function setApi(api: LaserApi, kind: ApiKind = 'mock'): void {
  current = api;
  currentKind = kind;
}
