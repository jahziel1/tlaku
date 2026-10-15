/**
 * Pide al motor las imágenes de la vista «por tonos» y las deja en `layers.ts` para el lienzo.
 *
 * Qué se pide y cuándo:
 * - Con el modo «por tonos» y una vista previa vigente, la imagen de toda la hoja (`full`).
 * - Al acercar o mover la vista, tras una breve pausa, el detalle de lo que se ve (`detail`); lo decide
 *   `planDetail`.
 * Cada petición lleva un número: una respuesta que llega tarde (la vista o los ajustes ya cambiaron) se
 * descarta. El motor guarda el último trabajo calculado, así que pedir otra zona no lo recalcula.
 */
import { useEffect } from 'react';
import { getApi } from '@/api';
import { hasTemplates, resolveJobArgs } from '@/features/variables/jobResolve';
import { useMaterials } from '@/features/materials/state';
import { errorMessage } from '@/lib/errors';
import { useStore } from '@/store';
import { buildJobArgs, selectActiveDevice } from '@/store/selectors';
import { laserWattsOf } from '@/lib/device';
import { curveFor, type ToneCurve } from './curve';
import { clearTonalLayers, getTonalLayers, setTonalLayers, type TonalLayer } from './layers';
import { FULL_MAX_PX, planDetail } from './plan';
import { useTonal } from './store';
import type { Box, TonalPreview, TonalRequest } from './types';

/** Pausa tras el último cambio de la vista antes de pedir el detalle (ms). */
export const DETAIL_DEBOUNCE_MS = 250;

const ids = new WeakMap<object, number>();
let nextId = 1;
/** Número que identifica a un objeto mientras viva (para saber si es el mismo cálculo). */
function identityOf(o: object): number {
  let id = ids.get(o);
  if (id === undefined) {
    id = nextId++;
    ids.set(o, id);
  }
  return id;
}

/** PNG en base64 a algo que el lienzo sabe dibujar. */
export async function decodePng(base64: string): Promise<CanvasImageSource> {
  const bin = atob(base64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  if (typeof createImageBitmap === 'function') return createImageBitmap(new Blob([bytes], { type: 'image/png' }));
  return new Promise<CanvasImageSource>((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error('png'));
    img.src = `data:image/png;base64,${base64}`;
  });
}

const layerOf = async (p: TonalPreview): Promise<TonalLayer> => ({ image: await decodePng(p.png_base64), covered: p.covered_mm, pxMm: p.px_mm });

async function request(req: TonalRequest): Promise<TonalPreview> {
  const base = buildJobArgs(useStore.getState());
  const args = hasTemplates(base.doc) ? await resolveJobArgs(base) : base;
  return getApi().tonalPreview({ ...args, request: req });
}

/** Curva y ancho de punto de ahora. */
function currentSettings(): { curve: ToneCurve; spotMm: number } {
  const { material, spotMm } = useTonal.getState();
  // Los materiales de partida van por watt: se ajustan al láser de la máquina activa (la calibración del usuario no).
  const watts = laserWattsOf(selectActiveDevice(useStore.getState()));
  return { curve: curveFor(material, useMaterials.getState().library.tones, watts), spotMm };
}

/** ¿La vista «por tonos» está activa y tiene una vista previa vigente de la que partir? */
export function tonalActive(): boolean {
  const s = useStore.getState();
  return useTonal.getState().mode === 'tonal' && s.previewVisible && s.preview !== null && !s.previewStale && s.preview.moves.length > 0;
}

export function startTonalService(): () => void {
  let fullSeq = 0;
  let detailSeq = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let fullKey = '';

  const cancel = () => {
    fullSeq++;
    detailSeq++;
    clearTimeout(timer);
  };

  /** Detalle de lo que se ve, si el zoom lo pide. */
  const refreshDetail = async () => {
    if (!tonalActive()) return;
    const { full, detail, sheet } = getTonalLayers();
    if (!full || !sheet) return;
    const s = useStore.getState();
    const plan = planDetail({
      view: { zoom: s.zoom, panX: s.panX, panY: s.panY },
      width: s.viewport.width,
      height: s.viewport.height,
      dpr: typeof window === 'undefined' ? 1 : window.devicePixelRatio || 1,
      sheet,
      fullPxMm: full.pxMm,
      detail: detail ? { covered: detail.covered, pxMm: detail.pxMm } : null,
    });
    if (plan.kind === 'keep') return;
    if (plan.kind === 'drop') {
      setTonalLayers({ detail: null });
      return;
    }
    const mine = ++detailSeq;
    const { curve, spotMm } = currentSettings();
    try {
      const p = await request({ curve, spot_mm: spotMm, region_mm: plan.region, max_px: plan.maxPx });
      const layer = await layerOf(p);
      if (mine === detailSeq && tonalActive()) setTonalLayers({ detail: layer });
    } catch {
      /* el detalle es un extra: si falla se sigue viendo la imagen general */
    }
  };

  const scheduleDetail = () => {
    clearTimeout(timer);
    timer = setTimeout(() => void refreshDetail(), DETAIL_DEBOUNCE_MS);
  };

  /** Toda la hoja; se pide de nuevo si cambió el trabajo, el material, la calibración o el punto. */
  const refreshFull = async () => {
    const ui = useTonal.getState();
    if (!tonalActive()) {
      cancel();
      clearTonalLayers();
      if (ui.status !== 'idle') ui.setStatus('idle');
      fullKey = '';
      return;
    }
    const { curve, spotMm } = currentSettings();
    const preview = useStore.getState().preview!;
    // El arreglo `moves` es nuevo con cada cálculo del trabajo (la potencia cambia el tono aunque no el largo).
    const key = JSON.stringify([curve, spotMm, identityOf(preview.moves)]);
    if (key === fullKey && getTonalLayers().full) return;
    fullKey = key;
    const mine = ++fullSeq;
    detailSeq++;
    ui.setStatus('busy');
    try {
      const p = await request({ curve, spot_mm: spotMm, region_mm: null, max_px: FULL_MAX_PX });
      const layer = await layerOf(p);
      if (mine !== fullSeq) return;
      setTonalLayers({ full: layer, detail: null, sheet: p.sheet_mm });
      useTonal.getState().setStatus('ready');
      scheduleDetail();
    } catch (e) {
      if (mine !== fullSeq) return;
      fullKey = '';
      clearTonalLayers();
      useTonal.getState().setStatus('error', errorMessage(e));
    }
  };

  const changed = () => void refreshFull();
  const unsubTonal = useTonal.subscribe((s, prev) => {
    if (s.mode !== prev.mode || s.material !== prev.material || s.spotMm !== prev.spotMm) changed();
  });
  const unsubMaterials = useMaterials.subscribe((s, prev) => {
    if (s.library.tones !== prev.library.tones) changed();
  });
  const unsubStore = useStore.subscribe((s, prev) => {
    if (s.preview !== prev.preview || s.previewStale !== prev.previewStale || s.previewVisible !== prev.previewVisible) changed();
    if (s.zoom !== prev.zoom || s.panX !== prev.panX || s.panY !== prev.panY || s.viewport !== prev.viewport) {
      if (useTonal.getState().mode === 'tonal') scheduleDetail();
    }
  });
  changed();
  return () => {
    unsubTonal();
    unsubMaterials();
    unsubStore();
    cancel();
    clearTonalLayers();
    fullKey = '';
    const ui = useTonal.getState();
    if (ui.status !== 'idle') ui.setStatus('idle');
  };
}

/** Mantiene las imágenes de la vista «por tonos» mientras la barra de la vista previa está en pantalla. */
export function useTonalSync(): void {
  useEffect(() => startTonalService(), []);
}

export type { Box };
