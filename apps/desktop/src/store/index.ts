import { create, type StoreApi, type UseBoundStore } from 'zustand';
import type { LaserApi } from '@/api';
import { createCameraSlice } from './camera';
import { createDevicesSlice } from './devices';
import { createDocumentSlice } from './document';
import { createGalvoSlice } from './galvo';
import { createMachineSlice } from './machine';
import { createMoveControlSlice } from './moveControl';
import { createNoticesSlice } from './notices';
import { createPreviewSlice } from './preview';
import { createPrefsSlice, type PrefKey, type Prefs } from './prefs';
import { createSelectionSlice } from './selection';
import { createTabsSlice } from './tabs';
import type { AppState } from './types';
import { createViewSlice } from './view';

export type { AppState } from './types';
export { DEFAULT_PREFS, PREF_KEYS, PREF_SPECS, type PrefKey, type Prefs } from './prefs';
export * from './selectors';
export { activeTabLabel, findTabByPath, isPristine, pinTab, tabDoc, tabLabel, tabViews, type TabEntry, type TabView, type JobOrigin } from './tabs';

/** Crea un store nuevo (la app usa `useStore`; las pruebas crean el suyo). */
export function createAppStore(): UseBoundStore<StoreApi<AppState>> {
  return create<AppState>()((...a) => ({
    ...createDocumentSlice(...a),
    ...createSelectionSlice(...a),
    ...createViewSlice(...a),
    ...createMachineSlice(...a),
    ...createGalvoSlice(...a),
    ...createMoveControlSlice(...a),
    ...createPreviewSlice(...a),
    ...createDevicesSlice(...a),
    ...createNoticesSlice(...a),
    ...createCameraSlice(...a),
    ...createPrefsSlice(...a),
    ...createTabsSlice(...a),
  }));
}

export const useStore = createAppStore();

/**
 * Preferencia del usuario como estado de React: `const snapPx = usePref('snapPx')`. El componente
 * solo se vuelve a pintar si cambia ese valor. El catálogo de claves está en `store/prefs.ts`.
 */
export function usePref<K extends PrefKey>(key: K): Prefs[K] {
  return useStore((s) => s.prefs[key]);
}

/** Preferencia leída fuera de React (comandos, interacción del lienzo). */
export function getPref<K extends PrefKey>(key: K): Prefs[K] {
  return useStore.getState().prefs[key];
}

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
    api.on('machine://galvo', (p) => s().onGalvo(p)),
  ]);
  return () => offs.forEach((off) => off());
}
