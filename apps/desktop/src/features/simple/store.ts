import { create } from 'zustand';
import type { SimpleAction, SimpleStep } from './model';
import { STEPS } from './model';

/**
 * Estado de interfaz del modo sencillo (no es parte del documento): en qué paso va el usuario, qué
 * material eligió y qué decidió para cada color. Lo que se aplica de verdad vive en las capas del
 * documento, así que pasar al modo completo y volver no pierde nada.
 */
interface SimpleUiState {
  step: number;
  /** Clave del material elegido (ver `materialChoices`), o `null`. */
  materialKey: string | null;
  /** Qué hacer con cada color (clave en minúsculas). Sin entrada = sin decidir. */
  actions: Record<string, SimpleAction>;
  /** Paso de "más claro / más oscuro" por color (0 = el valor del material); `null` = valores escritos a mano. */
  tune: Record<string, number | null>;
  setStep: (step: number) => void;
  setMaterialKey: (key: string | null) => void;
  setAction: (color: string, action: SimpleAction) => void;
  setTune: (color: string, tune: number | null) => void;
  clearTune: () => void;
  reset: () => void;
}

const INITIAL = { step: 0, materialKey: null, actions: {}, tune: {} };

export const useSimple = create<SimpleUiState>()((set) => ({
  ...INITIAL,
  setStep: (step) => set({ step: Math.max(0, Math.min(STEPS.length - 1, step)) }),
  setMaterialKey: (materialKey) => set({ materialKey }),
  setAction: (color, action) => set((s) => ({ actions: { ...s.actions, [color.toLowerCase()]: action } })),
  setTune: (color, tune) => set((s) => ({ tune: { ...s.tune, [color.toLowerCase()]: tune } })),
  clearTune: () => set({ tune: {} }),
  reset: () => set({ ...INITIAL }),
}));

export const stepName = (step: number): SimpleStep => STEPS[Math.max(0, Math.min(STEPS.length - 1, step))]!;
