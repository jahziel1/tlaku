import { create, type StoreApi, type UseBoundStore } from 'zustand';
import type { LaserApi } from '@/api';
import { createDevicesSlice } from './devices';
import { createDocumentSlice } from './document';
import { createMachineSlice } from './machine';
import { createNoticesSlice } from './notices';
import { createPreviewSlice } from './preview';
import { createSelectionSlice } from './selection';
import type { AppState } from './types';
import { createViewSlice } from './view';

export type { AppState } from './types';
export * from './selectors';

/** Crea un store nuevo (la app usa `useStore`; las pruebas crean el suyo). */
export function createAppStore(): UseBoundStore<StoreApi<AppState>> {
  return create<AppState>()((...a) => ({
    ...createDocumentSlice(...a),
    ...createSelectionSlice(...a),
    ...createViewSlice(...a),
    ...createMachineSlice(...a),
    ...createPreviewSlice(...a),
    ...createDevicesSlice(...a),
    ...createNoticesSlice(...a),
  }));
}

export const useStore = createAppStore();

/**
 * Conecta los eventos `machine://*` del motor con el slice de la máquina.
 * Devuelve la función que cancela todas las suscripciones.
 */
export async function bindMachineEvents(
  api: LaserApi,
  store: Pick<UseBoundStore<StoreApi<AppState>>, 'getState'> = useStore,
): Promise<() => void> {
  const s = () => store.getState();
  const offs = await Promise.all([
    api.on('machine://status', (p) => s().onStatus(p)),
    api.on('machine://progress', (p) => s().onProgress(p)),
    api.on('machine://log', (p) => s().onLog(p)),
    api.on('machine://job', (p) => s().onJob(p)),
    api.on('machine://connection', (p) => s().onConnection(p)),
  ]);
  return () => offs.forEach((off) => off());
}
