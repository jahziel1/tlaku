import { create } from 'zustand';
import type { Zoom } from './view';

/** Estado del panel de vista previa de imágenes (no es parte del documento). */
interface PreviewUiState {
  open: boolean;
  /** Imagen elegida; `null` = la seleccionada de la capa o la primera. */
  imageId: string | null;
  zoom: Zoom;
  /** Centro de la vista a escala real, mm locales de la imagen; `null` = el centro de la imagen. */
  center: [number, number] | null;
  set: (patch: Partial<Omit<PreviewUiState, 'set'>>) => void;
}

export const usePreviewUi = create<PreviewUiState>()((set) => ({
  open: false,
  imageId: null,
  zoom: 'fit',
  center: null,
  set: (patch) => set(patch),
}));
