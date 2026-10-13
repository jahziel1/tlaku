import { create } from 'zustand';
import { DEFAULT_BITMAP_OPTIONS } from './bitmap';
import type { BitmapOptions } from './types';

interface BitmapDialogState {
  open: boolean;
  /** Se recuerdan entre aperturas durante la sesión. */
  options: BitmapOptions;
  replace: boolean;
  show: () => void;
  close: () => void;
  patch: (patch: Partial<BitmapOptions>) => void;
  setReplace: (v: boolean) => void;
}

/** Estado de interfaz del diálogo "Convertir a mapa de bits" (no es parte del documento). */
export const useBitmapDialog = create<BitmapDialogState>()((set) => ({
  open: false,
  options: DEFAULT_BITMAP_OPTIONS,
  replace: true,
  show: () => set({ open: true }),
  close: () => set({ open: false }),
  patch: (patch) => set((s) => ({ options: { ...s.options, ...patch } })),
  setReplace: (replace) => set({ replace }),
}));
