import { create } from 'zustand';
import type { PanelPos } from './position';

/** Estado de la interfaz de «Revisar diseño». Los hallazgos no se guardan aquí: salen de `useReview`. */
interface ReviewUiState {
  open: boolean;
  /** Claves de los hallazgos que el usuario pidió ignorar (valen mientras dure la sesión). */
  ignored: string[];
  showIgnored: boolean;
  /** Un arreglo está en marcha: los botones esperan. */
  busy: boolean;
  /** Dónde dejó el usuario el panel (se recuerda mientras dure la sesión); null = la posición por omisión. */
  pos: PanelPos | null;
  set: (patch: Partial<Pick<ReviewUiState, 'open' | 'showIgnored' | 'busy' | 'pos'>>) => void;
  ignore: (key: string) => void;
  restore: (key: string) => void;
}

export const useReviewUi = create<ReviewUiState>()((set) => ({
  open: false,
  ignored: [],
  showIgnored: false,
  busy: false,
  pos: null,
  set: (patch) => set(patch),
  ignore: (key) => set((s) => (s.ignored.includes(key) ? s : { ignored: [...s.ignored, key] })),
  restore: (key) => set((s) => ({ ignored: s.ignored.filter((k) => k !== key) })),
}));

/** Abre el panel «Revisar diseño». */
export function openReview(): void {
  useReviewUi.getState().set({ open: true });
}

export function closeReview(): void {
  useReviewUi.getState().set({ open: false });
}
