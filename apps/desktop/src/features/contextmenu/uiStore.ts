import { create } from 'zustand';
import type { Point } from '@/api/types';

/** Dónde y sobre qué se abrió el menú contextual del lienzo. */
export interface ContextMenuState {
  /** Posición en pantalla (px de la ventana). */
  x: number;
  y: number;
  /** Punto del documento bajo el cursor (mm). */
  at: Point;
  /** Hay objetos seleccionados (el clic cayó sobre uno, o ya había una selección). */
  onObject: boolean;
}

interface ContextMenuUi {
  menu: ContextMenuState | null;
  open: (menu: ContextMenuState) => void;
  close: () => void;
}

export const useContextMenu = create<ContextMenuUi>()((set) => ({
  menu: null,
  open: (menu) => set({ menu }),
  close: () => set((s) => (s.menu ? { menu: null } : s)),
}));

export const openContextMenu = (menu: ContextMenuState): void => useContextMenu.getState().open(menu);
export const closeContextMenu = (): void => useContextMenu.getState().close();
