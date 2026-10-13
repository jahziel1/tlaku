import { create } from 'zustand';

interface ShellUi {
  shortcutsOpen: boolean;
  openShortcuts: () => void;
  closeShortcuts: () => void;
}

/** Estado de interfaz propio del shell (diálogos). Aparte del store de la app porque no es parte del documento. */
export const useShellUi = create<ShellUi>()((set) => ({
  shortcutsOpen: false,
  openShortcuts: () => set({ shortcutsOpen: true }),
  closeShortcuts: () => set({ shortcutsOpen: false }),
}));
