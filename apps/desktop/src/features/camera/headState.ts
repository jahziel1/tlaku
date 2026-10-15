/** Estado de interfaz de la cámara en el cabezal: confirmación, avance del recorrido y calibración. */
import { create } from 'zustand';
import type { Vec2 } from './geometry';
import type { HeadObservation } from './head';

/** Lo que se pide hacer con el recorrido; todas mueven la cabeza, así que piden confirmación. */
export type HeadAction = 'overlay' | 'capture' | 'vectorize';

export interface ScanProgress {
  step: number;
  total: number;
}

export interface MosaicInfo {
  tiles: number;
  coverage: number;
  partial: boolean;
}

/** Rectángulo en mm de documento: x0, y0, x1, y1 (con x0 < x1 y y0 < y1). */
export type ZoneRect = [number, number, number, number];

interface HeadUiState {
  /** Acción esperando que el usuario confirme que la cabeza se va a mover. */
  confirm: HeadAction | null;
  /** Limitar el recorrido a la selección. */
  useSelection: boolean;
  /** Zona dibujada en el lienzo para el recorrido (mm de documento), o `null`. */
  zone: ZoneRect | null;
  /** Limitar el recorrido a la zona dibujada. Excluye a `useSelection`. */
  useZone: boolean;
  /** El usuario está dibujando la zona sobre el lienzo. */
  drawingZone: boolean;
  progress: ScanProgress | null;
  cancelRequested: boolean;
  mosaic: MosaicInfo | null;
  error: string | null;
  ask: (a: HeadAction | null) => void;
  setUseSelection: (v: boolean) => void;
  setZone: (z: ZoneRect | null) => void;
  setUseZone: (v: boolean) => void;
  setDrawingZone: (v: boolean) => void;
  setProgress: (p: ScanProgress | null) => void;
  requestCancel: (v: boolean) => void;
  setMosaic: (m: MosaicInfo | null) => void;
  setError: (e: string | null) => void;
}

export const useHeadUi = create<HeadUiState>()((set) => ({
  confirm: null,
  useSelection: false,
  zone: null,
  useZone: false,
  drawingZone: false,
  progress: null,
  cancelRequested: false,
  mosaic: null,
  error: null,
  ask: (confirm) => set({ confirm, error: null }),
  setUseSelection: (useSelection) => set((s) => ({ useSelection, useZone: useSelection ? false : s.useZone })),
  // Dibujar una zona la deja elegida; borrarla vuelve a toda la cama.
  setZone: (zone) => set((s) => ({ zone, useZone: zone !== null, useSelection: zone !== null ? false : s.useSelection })),
  setUseZone: (useZone) => set((s) => ({ useZone: useZone && s.zone !== null, useSelection: useZone && s.zone !== null ? false : s.useSelection })),
  setDrawingZone: (drawingZone) => set({ drawingZone }),
  setProgress: (progress) => set({ progress }),
  requestCancel: (cancelRequested) => set({ cancelRequested }),
  setMosaic: (mosaic) => set({ mosaic }),
  setError: (error) => set({ error }),
}));

/** Distancia (mm) de los pasos de la calibración del cabezal. */
export const HEAD_CAL_STEP_MM = 6;
export const HEAD_CAL_MIN_OBS = 3;
export const HEAD_CAL_MAX_OBS = 6;

interface HeadCalState {
  /** Posición (mm) de la marca grabada. */
  mark: Vec2 | null;
  /** Midiendo: cada clic en la imagen registra una observación. */
  measuring: boolean;
  obs: HeadObservation[];
  setMark: (m: Vec2 | null) => void;
  setMeasuring: (v: boolean) => void;
  addObs: (o: HeadObservation) => void;
  reset: () => void;
}

export const useHeadCal = create<HeadCalState>()((set) => ({
  mark: null,
  measuring: false,
  obs: [],
  setMark: (mark) => set({ mark }),
  setMeasuring: (measuring) => set({ measuring }),
  addObs: (o) => set((s) => ({ obs: [...s.obs, o].slice(-HEAD_CAL_MAX_OBS) })),
  reset: () => set({ obs: [], measuring: false }),
}));

/** Herramienta que atiende los clics sobre la imagen del panel. */
export type ViewTool = 'align' | 'headMark' | 'center' | 'alignRef';

interface ViewToolState {
  tool: ViewTool;
  setTool: (t: ViewTool) => void;
}

export const useViewTool = create<ViewToolState>()((set) => ({
  tool: 'align',
  setTool: (tool) => set({ tool }),
}));
