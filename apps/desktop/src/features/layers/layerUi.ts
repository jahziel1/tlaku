import { create } from 'zustand';
import type { LayerTemplate } from '@/lib/layers';
import { readPref, writePref } from '@/lib/platform';

/**
 * Estado de interfaz propio del panel de capas (no es parte del documento): qué capa se edita
 * y cuál se está renombrando. Vive aquí, y no en `store/`, porque nadie más lo necesita.
 */
interface LayerUiState {
  /** Color de la capa cuyo editor de ajustes está abierto. */
  editing: string | null;
  /** Color de la capa que se renombra en la lista. */
  renaming: string | null;
  /** Mostrar la ayuda breve bajo cada parámetro. */
  showHelp: boolean;
  /** Ajustes copiados de una capa (C06), listos para pegar en otra, y el nombre de la capa de origen. */
  copied: { template: LayerTemplate; from: string } | null;
  openEditor: (color: string) => void;
  closeEditor: () => void;
  startRename: (color: string) => void;
  stopRename: () => void;
  setShowHelp: (v: boolean) => void;
  setCopied: (template: LayerTemplate, from: string) => void;
}

export const useLayerUi = create<LayerUiState>()((set) => ({
  editing: null,
  renaming: null,
  showHelp: readPref('layers.help') !== 'off',
  copied: null,
  openEditor: (editing) => set({ editing, renaming: null }),
  closeEditor: () => set({ editing: null }),
  startRename: (renaming) => set({ renaming }),
  stopRename: () => set({ renaming: null }),
  setCopied: (template, from) => set({ copied: { template, from } }),
  setShowHelp: (showHelp) => {
    writePref('layers.help', showHelp ? 'on' : 'off');
    set({ showHelp });
  },
}));

/** Devuelve el foco al botón principal de una fila (tras cerrar el editor o mover una capa). */
export function focusLayerRow(color: string): void {
  requestAnimationFrame(() => {
    const rows = document.querySelectorAll<HTMLElement>('[data-layer-main]');
    for (const el of rows) {
      if (el.dataset.layerMain === color) {
        el.focus();
        return;
      }
    }
  });
}
