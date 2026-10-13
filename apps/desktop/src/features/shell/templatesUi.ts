import { create } from 'zustand';

/** Diálogo de plantillas abierto: guardar el proyecto actual como plantilla o elegir una para empezar. */
export type TemplatesDialog = 'save' | 'new' | null;

interface TemplatesUi {
  dialog: TemplatesDialog;
  open: (dialog: Exclude<TemplatesDialog, null>) => void;
  close: () => void;
}

export const useTemplatesUi = create<TemplatesUi>()((set) => ({
  dialog: null,
  open: (dialog) => set({ dialog }),
  close: () => set({ dialog: null }),
}));
