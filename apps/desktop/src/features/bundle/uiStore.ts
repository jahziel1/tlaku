import { create } from 'zustand';

export type BundleMode = 'export' | 'import' | 'backup';

/** De dónde viene el paquete que se abre solo: un respaldo de la carpeta (su marca sirve para no volver a ofrecerlo). */
export interface ImportSource {
  path: string;
  /** Marca del respaldo (ms desde 1970); al aplicar se anota como ya visto. */
  createdMs?: number;
}

interface BundleUi {
  /** Diálogo abierto, o `null`. */
  mode: BundleMode | null;
  /** Paquete que el diálogo de importar abre solo (un respaldo elegido o ofrecido). */
  source: ImportSource | null;
  openBundle: (mode: BundleMode) => void;
  /** Abre «Importar» con ese paquete ya leído y su vista previa. */
  openImportFrom: (source: ImportSource) => void;
  closeBundle: () => void;
}

/** Estado de interfaz del diálogo de paquetes de configuración (abierto o no, y para qué). */
export const useBundleUi = create<BundleUi>()((set) => ({
  mode: null,
  source: null,
  openBundle: (mode) => set({ mode, source: null }),
  openImportFrom: (source) => set({ mode: 'import', source }),
  closeBundle: () => set({ mode: null, source: null }),
}));
