import { create } from 'zustand';
import type { Affine, Point } from '@/api/types';
import type { ModeChoice } from './flow';
import type { DesignMark, MarkCount, MarkKind } from './marks';
import type { FitWarning } from './types';

export type PrintCutStep = 'marks' | 'capture' | 'result';
export type CaptureMethod = 'laser' | 'camera';

/** Resumen de la transformación aceptada, para mostrarla mientras el trabajo la use. */
export interface AppliedRegistration {
  transform: Affine;
  marks: number;
  rmsMm: number;
  maxMm: number;
  rotationDeg: number;
  scale: number;
  warnings: FitWarning[];
}

interface PrintCutUiState {
  open: boolean;
  step: PrintCutStep;
  /** Marcas del diseño elegidas (paso 1). */
  marks: DesignMark[];
  /** Posición física medida de cada marca, en mm de la cama (paso 2). */
  physical: (Point | null)[];
  /** Marca que se está midiendo. */
  current: number;
  method: CaptureMethod;
  /** Esperando un clic sobre la imagen de la cámara. */
  picking: boolean;
  mode: ModeChoice;
  markKind: MarkKind;
  markCount: MarkCount;
  applied: AppliedRegistration | null;

  show: () => void;
  close: () => void;
  patch: (patch: Partial<Omit<PrintCutUiState, 'show' | 'close' | 'patch' | 'setMarks' | 'setPhysical' | 'restart'>>) => void;
  /** Cambia las marcas del diseño y descarta las mediciones (ya no valen). */
  setMarks: (marks: DesignMark[]) => void;
  setPhysical: (index: number, point: Point | null) => void;
  /** Vuelve al paso 1 sin perder las marcas, descartando las mediciones. */
  restart: () => void;
}

/** Estado de interfaz del asistente Print & Cut (no es parte del documento). */
export const usePrintCut = create<PrintCutUiState>()((set) => ({
  open: false,
  step: 'marks',
  marks: [],
  physical: [],
  current: 0,
  method: 'laser',
  picking: false,
  mode: 'auto',
  markKind: 'cross',
  markCount: 2,
  applied: null,

  show: () => set({ open: true }),
  close: () => set({ open: false, picking: false }),
  patch: (patch) => set(patch),
  setMarks: (marks) => set({ marks, physical: marks.map(() => null), current: 0, picking: false }),
  setPhysical: (index, point) =>
    set((s) => {
      const physical = s.physical.slice();
      physical[index] = point;
      return { physical };
    }),
  restart: () => set((s) => ({ step: 'marks', physical: s.marks.map(() => null), current: 0, picking: false })),
}));
