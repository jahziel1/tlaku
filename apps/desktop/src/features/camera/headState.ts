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

interface HeadUiState {
  /** Acción esperando que el usuario confirme que la cabeza se va a mover. */
  confirm: HeadAction | null;
  /** Limitar el recorrido a la selección. */
  useSelection: boolean;
  progress: ScanProgress | null;
  cancelRequested: boolean;
  mosaic: MosaicInfo | null;
  error: string | null;
  ask: (a: HeadAction | null) => void;
  setUseSelection: (v: boolean) => void;
  setProgress: (p: ScanProgress | null) => void;
  requestCancel: (v: boolean) => void;
  setMosaic: (m: MosaicInfo | null) => void;
  setError: (e: string | null) => void;
}

export const useHeadUi = create<HeadUiState>()((set) => ({
  confirm: null,
  useSelection: false,
  progress: null,
  cancelRequested: false,
  mosaic: null,
  error: null,
  ask: (confirm) => set({ confirm, error: null }),
  setUseSelection: (useSelection) => set({ useSelection }),
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
export type ViewTool = 'align' | 'headMark' | 'center';

interface ViewToolState {
  tool: ViewTool;
  setTool: (t: ViewTool) => void;
}

export const useViewTool = create<ViewToolState>()((set) => ({
  tool: 'align',
  setTool: (tool) => set({ tool }),
}));
