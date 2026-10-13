import { create } from 'zustand';
import type { CodeSpec } from '@/api/types';
import { DEFAULT_CODE_SPEC } from './kinds';

interface CodesDialogState {
  open: boolean;
  /** Se recuerda entre aperturas durante la sesión (el contenido, no). */
  spec: CodeSpec;
  show: () => void;
  close: () => void;
  patch: (patch: Partial<CodeSpec>) => void;
}

/** Estado de interfaz del diálogo "Insertar código" (no es parte del documento). */
export const useCodesDialog = create<CodesDialogState>()((set) => ({
  open: false,
  spec: DEFAULT_CODE_SPEC,
  show: () => set((s) => ({ open: true, spec: { ...s.spec, data: '' } })),
  close: () => set({ open: false }),
  patch: (patch) => set((s) => ({ spec: { ...s.spec, ...patch } })),
}));
