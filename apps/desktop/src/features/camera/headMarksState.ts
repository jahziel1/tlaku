/** Estado de interfaz de la alineación automática del cabezal con marcas grabadas. */
import { create } from 'zustand';
import type { Vec2 } from './geometry';
import type { HeadObservation } from './head';

/** Lado del cuadro de marcas (mm): el mínimo que acepta el motor y el máximo útil. */
export const HEAD_MARKS_MIN_SIZE = 60;
export const HEAD_MARKS_MAX_SIZE = 400;
export const HEAD_MARKS_DEFAULT_SIZE = 80;
/** Marcas mínimas usadas para fiarse de una detección. */
export const HEAD_MARKS_MIN_USED = 4;
/** Tope de observaciones acumuladas (varias posiciones del cabezal). */
export const HEAD_MARKS_MAX_OBS = 90;

export interface HeadMarksDetection {
  /** Posición del láser al detectar (mm). */
  laser: Vec2;
  found: number;
  used: number;
}

interface HeadMarksState {
  size: number;
  /** Esquina inferior izquierda del cuadro de marcas (mm); `null` = centrado en la cama. */
  origin: Vec2 | null;
  obs: HeadObservation[];
  detections: HeadMarksDetection[];
  busy: boolean;
  error: string | null;
  setSize: (v: number) => void;
  setOrigin: (v: Vec2 | null) => void;
  add: (obs: HeadObservation[], detection: HeadMarksDetection) => void;
  setBusy: (v: boolean) => void;
  setError: (e: string | null) => void;
  reset: () => void;
}

export const useHeadMarks = create<HeadMarksState>()((set) => ({
  size: HEAD_MARKS_DEFAULT_SIZE,
  origin: null,
  obs: [],
  detections: [],
  busy: false,
  error: null,
  setSize: (size) => set({ size, obs: [], detections: [] }),
  setOrigin: (origin) => set({ origin, obs: [], detections: [] }),
  add: (obs, detection) =>
    set((s) => ({ obs: [...s.obs, ...obs].slice(-HEAD_MARKS_MAX_OBS), detections: [...s.detections, detection].slice(-10), error: null })),
  setBusy: (busy) => set({ busy }),
  setError: (error) => set({ error }),
  reset: () => set({ obs: [], detections: [], busy: false, error: null }),
}));
