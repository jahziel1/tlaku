import { create } from 'zustand';
import { selectActiveDevice, selectJobSelection, useStore } from '@/store';
import { designBounds } from '@/features/machine/placement';
import { defaultTileSize, planTiles, shiftTiles, type Bounds, type PlanResult, type PlannedTile } from './plan';
import { isComplete, loadSession, sessionKey, type TileSession, type TilingConfig } from './session';

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
  /** Plan con las opciones actuales (sin el margen aplicado). */
  plan: PlanResult | null;
  session: TileSession | null;
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

/** Plan de las opciones sobre la caja del diseño; `null` si no hay diseño. */
export function computePlan(design: Bounds | null, config: TilingConfig): PlanResult | null {
  return design ? planTiles(design, config.size, config.overlap, config.order) : null;
}

/** Tramos listos para enviar: con el margen de la cama sumado al desplazamiento. */
export function plannedWithMargin(tiles: PlannedTile[], margin: number): PlannedTile[] {
  return shiftTiles(tiles, [margin, margin]);
}

export const useTilingUi = create<TilingUiState>()((set, get) => ({
  open: false,
  phase: 'config',
  config: defaultConfig([400, 400]),
  design: null,
  plan: null,
  session: null,
  saved: null,
  message: null,
  set: (patch) => set(patch),
  patchConfig: (patch) => {
    const config = { ...get().config, ...patch };
    set({ config, plan: computePlan(get().design, config) });
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
  ui.set({ open: true, phase: 'config', design, config, plan: computePlan(design, config), session: null, saved, message: null });
}

/** Cierra el panel. Una sesión a medias queda guardada y el lienzo deja de mostrar los tramos. */
export function closeTiling(): void {
  useTilingUi.getState().set({ open: false });
}
