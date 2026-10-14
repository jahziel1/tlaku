import { create } from 'zustand';
import { getApi } from '@/api';
import { errorMessage } from '@/lib/errors';
import { selectActiveDevice, selectJobSelection, useStore } from '@/store';
import { designBounds } from '@/features/machine/placement';
import { defaultTileSize } from './plan';
import { isComplete, loadSession, sessionKey, type TileSession, type TilingConfig } from './session';
import type { Bounds, MarkSegment, PlanResult } from './types';

/**
 * Fase del asistente:
 * - `config`: se eligen tamaño, solape, orden y marcas (el lienzo muestra los tramos);
 * - `place`: "Coloca el material así…", espera la confirmación para enviar el tramo actual;
 * - `sending`: el tramo está en la máquina;
 * - `feeding`: el alimentador automático avanza el material;
 * - `failed`: el tramo no salió (error, cancelación o máquina no lista);
 * - `finished`: ya no queda ningún tramo pendiente.
 */
export type TilingPhase = 'config' | 'place' | 'sending' | 'feeding' | 'failed' | 'finished';

export interface TilingUiState {
  open: boolean;
  phase: TilingPhase;
  config: TilingConfig;
  /** Caja del diseño que se divide (con «Solo la selección», la de la selección). */
  design: Bounds | null;
  /**
   * Plan que calculó el motor para `design` y `config` (los desplazamientos ya llevan el margen).
   * Mientras `planning` es `true` es el de las opciones anteriores: no hay que enviarlo.
   */
  plan: PlanResult | null;
  /** Se está pidiendo al motor el plan de las opciones actuales. */
  planning: boolean;
  session: TileSession | null;
  /** Cruces de alineación de la sesión (para las marcas de registro de la guía). */
  marks: MarkSegment[];
  /** La guía «mueve el material» está abierta. */
  guide: boolean;
  /** Avance guardado de una sesión interrumpida que se puede retomar. */
  saved: TileSession | null;
  message: string | null;
  set: (patch: Partial<TilingUiState>) => void;
  patchConfig: (patch: Partial<TilingConfig>) => void;
}

export const DEFAULT_MARGIN_MM = 5;
export const DEFAULT_OVERLAP_MM = 5;

export function defaultConfig(bed: readonly [number, number]): TilingConfig {
  const margin = Math.min(DEFAULT_MARGIN_MM, Math.floor(Math.min(bed[0], bed[1]) / 4));
  return { size: defaultTileSize(bed, margin), overlap: DEFAULT_OVERLAP_MM, order: 'rows', marks: true, margin };
}

/** Pide al motor el plan de las opciones sobre la caja del diseño. */
export function requestPlan(design: Bounds, config: TilingConfig): Promise<PlanResult> {
  return getApi().planTiles({ design, size: config.size, overlap: config.overlap, order: config.order, margin: config.margin });
}

let planSeq = 0;

/**
 * Recalcula el plan con el diseño y las opciones de ahora. Si llegan varias peticiones juntas solo
 * vale la última: las demás se descartan al responder.
 */
export function refreshPlan(): void {
  const seq = ++planSeq;
  const { design, config, set } = useTilingUi.getState();
  if (!design) return set({ plan: null, planning: false });
  set({ planning: true });
  requestPlan(design, config).then(
    (plan) => {
      if (seq === planSeq) set({ plan, planning: false });
    },
    (e: unknown) => {
      if (seq !== planSeq) return;
      set({ plan: null, planning: false });
      useStore.getState().notify('error', errorMessage(e));
    },
  );
}

export const useTilingUi = create<TilingUiState>()((set, get) => ({
  open: false,
  phase: 'config',
  config: defaultConfig([400, 400]),
  design: null,
  plan: null,
  planning: false,
  session: null,
  marks: [],
  guide: false,
  saved: null,
  message: null,
  set: (patch) => set(patch),
  patchConfig: (patch) => {
    set({ config: { ...get().config, ...patch } });
    refreshPlan();
  },
}));

/** Lee el diseño y la máquina de ahora y deja el asistente en `config` (o retoma la sesión en curso). */
export function openTiling(): void {
  const ui = useTilingUi.getState();
  // Con una sesión en marcha (se cerró el panel a medias) se vuelve donde estaba.
  if (ui.session && ui.phase !== 'config') return ui.set({ open: true });
  const app = useStore.getState();
  const device = selectActiveDevice(app);
  const design = designBounds(app.doc, selectJobSelection(app));
  const stored = loadSession();
  const saved = stored && design && !isComplete(stored) && sessionKey(design, stored.config, device.id) === stored.key ? stored : null;
  const config = saved?.config ?? defaultConfig(device.bed);
  ui.set({ open: true, phase: 'config', design, config, plan: null, session: null, saved, message: null });
  refreshPlan();
}

/** Cierra el panel. Una sesión a medias queda guardada y el lienzo deja de mostrar los tramos. */
export function closeTiling(): void {
  useTilingUi.getState().set({ open: false });
}
