import { create } from 'zustand';
import { DEFAULT_NEST_OPTIONS, type NestOptions } from './request';

interface NestUiState {
  open: boolean;
  /** Se recuerdan entre aperturas durante la sesión. */
  options: NestOptions;
  show: () => void;
  close: () => void;
  patch: (patch: Partial<NestOptions>) => void;
}

/** Estado de interfaz del diálogo de nesting (no es parte del documento). */
export const useNestUi = create<NestUiState>()((set) => ({
  open: false,
  options: DEFAULT_NEST_OPTIONS,
  show: () => set({ open: true }),
  close: () => set({ open: false }),
  patch: (patch) => set((s) => ({ options: { ...s.options, ...patch } })),
}));
