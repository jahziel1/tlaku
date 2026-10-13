import type { StoreApi } from 'zustand';
import { selectActiveDevice, type AppState } from '@/store';

/** Espera (ms) tras el último cambio antes de recalcular. Oculta la vista previa, se espera más. */
export const SYNC_DELAYS = { visibleMs: 400, hiddenMs: 1500 } as const;

type SyncStore = Pick<StoreApi<AppState>, 'getState' | 'subscribe'>;

/**
 * Mantiene al día el cálculo del trabajo:
 *  - cada cambio del documento (objetos y capas), del dispositivo activo o de las opciones del
 *    trabajo invalida el cálculo en vuelo (su respuesta se descarta) y programa uno nuevo con
 *    debounce;
 *  - al abrir la vista previa se calcula de inmediato si hace falta;
 *  - con la vista previa cerrada el recálculo solo sirve a la estimación de la barra de estado,
 *    así que espera más y no corre por cada pulsación.
 * Devuelve la función que lo detiene.
 */
export function startPreviewSync(store: SyncStore, delays: { visibleMs: number; hiddenMs: number } = SYNC_DELAYS): () => void {
  let timer: ReturnType<typeof setTimeout> | undefined;

  const run = () => {
    timer = undefined;
    const s = store.getState();
    if (s.previewStale || s.previewStatus === 'idle') void s.computePreview();
  };
  const schedule = (ms: number) => {
    clearTimeout(timer);
    timer = setTimeout(run, ms);
  };

  const unsub = store.subscribe((s, prev) => {
    const changed =
      s.doc !== prev.doc || s.jobOptions !== prev.jobOptions || selectActiveDevice(s) !== selectActiveDevice(prev);
    if (changed) {
      s.invalidatePreview();
      schedule(s.previewVisible ? delays.visibleMs : delays.hiddenMs);
    } else if (s.previewVisible && !prev.previewVisible && (s.previewStale || s.previewStatus === 'idle')) {
      clearTimeout(timer);
      run();
    }
  });
  schedule(store.getState().previewVisible ? delays.visibleMs : delays.hiddenMs);

  return () => {
    clearTimeout(timer);
    unsub();
  };
}
