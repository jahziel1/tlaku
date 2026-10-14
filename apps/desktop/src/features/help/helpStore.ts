import { create } from 'zustand';
import type { MaterialPreset } from '@/api/types';
import { INDEX_FILE } from './pages';

export type HelpDialog = 'manual' | 'news' | 'about' | 'report' | 'donate' | 'jobLog' | 'update' | 'shareMachine' | 'shareMaterial';

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
  /** Ajuste propio que se comparte (diálogo «Compartir ajustes de material»); `null` = ninguno. */
  sharePreset: MaterialPreset | null;
  openShareMaterial: (preset?: MaterialPreset | null) => void;
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
  sharePreset: null,
  openShareMaterial: (preset = null) => set({ dialog: 'shareMaterial', sharePreset: preset }),
  close: () => set({ dialog: null, sharePreset: null }),
}));
