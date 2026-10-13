import { create } from 'zustand';
import type { TraceSettings } from '@/api/types';
import { DEFAULT_TRACE_SETTINGS } from './settings';

interface TraceDialogState {
  /** Imagen que se está vectorizando; `null` con el diálogo cerrado. */
  imageId: string | null;
  /** Se recuerdan entre aperturas durante la sesión. */
  settings: TraceSettings;
  deleteImage: boolean;
  showImage: boolean;
  open: (imageId: string) => void;
  close: () => void;
  patchSettings: (patch: Partial<TraceSettings>) => void;
  resetSettings: () => void;
  setDeleteImage: (v: boolean) => void;
  setShowImage: (v: boolean) => void;
}

/** Estado de interfaz del diálogo (no es parte del documento). */
export const useTraceDialog = create<TraceDialogState>()((set) => ({
  imageId: null,
  settings: DEFAULT_TRACE_SETTINGS,
  deleteImage: false,
  showImage: true,
  open: (imageId) => set({ imageId }),
  close: () => set({ imageId: null }),
  patchSettings: (patch) => set((s) => ({ settings: { ...s.settings, ...patch } })),
  resetSettings: () => set({ settings: DEFAULT_TRACE_SETTINGS }),
  setDeleteImage: (deleteImage) => set({ deleteImage }),
  setShowImage: (showImage) => set({ showImage }),
}));
