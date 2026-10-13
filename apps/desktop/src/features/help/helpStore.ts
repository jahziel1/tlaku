import { create } from 'zustand';
import { INDEX_FILE } from './pages';

export type HelpDialog = 'manual' | 'news' | 'about' | 'report' | 'donate';

interface HelpUi {
  dialog: HelpDialog | null;
  /** Página del manual que se está leyendo (nombre de archivo). */
  page: string;
  /** Sección a la que bajar al mostrar la página (id de un título). */
  anchor: string;
  open: (dialog: HelpDialog) => void;
  /** Abre el manual en una página (y sección) concretas; el resto de la app lo usa para ayuda contextual. */
  openManual: (page?: string, anchor?: string) => void;
  goTo: (page: string, anchor?: string) => void;
  close: () => void;
}

/** Estado de interfaz de la ayuda (qué diálogo está abierto y qué página del manual se lee). */
export const useHelpUi = create<HelpUi>()((set) => ({
  dialog: null,
  page: INDEX_FILE,
  anchor: '',
  open: (dialog) => set({ dialog }),
  openManual: (page, anchor = '') => set((s) => ({ dialog: 'manual', page: page ?? s.page, anchor })),
  goTo: (page, anchor = '') => set({ page, anchor }),
  close: () => set({ dialog: null }),
}));
