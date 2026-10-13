import { create } from 'zustand';

export type BundleMode = 'export' | 'import';

interface BundleUi {
  /** Diálogo abierto, o `null`. */
  mode: BundleMode | null;
  openBundle: (mode: BundleMode) => void;
  closeBundle: () => void;
}

/** Estado de interfaz del diálogo de paquetes de configuración (abierto o no, y para qué). */
export const useBundleUi = create<BundleUi>()((set) => ({
  mode: null,
  openBundle: (mode) => set({ mode }),
  closeBundle: () => set({ mode: null }),
}));
