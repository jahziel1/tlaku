import { create } from 'zustand';
import { DEFAULT_VERSIONS_CONFIG, type VersionsConfig } from './types';

interface VersionsUi {
  /** Diálogo «Historial de versiones» abierto. */
  open: boolean;
  /** Proyecto cuyo historial se muestra; `null` = el documento activo no tiene archivo (solo se ofrecen otros proyectos). */
  path: string | null;
  /** Diálogo «Guardar versión…» abierto. */
  nameOpen: boolean;
  /** Configuración vigente (la lee y la cambia el diálogo; el temporizador de instantáneas la sigue). */
  config: VersionsConfig;
  /** Sube cuando el historial cambia desde fuera del diálogo (guardar una versión con nombre): el diálogo abierto recarga. */
  changes: number;
  openFor: (path: string | null) => void;
  close: () => void;
  openName: () => void;
  closeName: () => void;
  setConfig: (config: VersionsConfig) => void;
  bump: () => void;
}

/** Estado de la interfaz del historial de versiones (qué diálogo está abierto y la configuración). */
export const useVersionsUi = create<VersionsUi>()((set) => ({
  open: false,
  path: null,
  nameOpen: false,
  config: DEFAULT_VERSIONS_CONFIG,
  changes: 0,
  openFor: (path) => set({ open: true, path }),
  close: () => set({ open: false }),
  openName: () => set({ nameOpen: true }),
  closeName: () => set({ nameOpen: false }),
  setConfig: (config) => set({ config }),
  bump: () => set((s) => ({ changes: s.changes + 1 })),
}));
