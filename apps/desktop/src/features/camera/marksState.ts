import { create } from 'zustand';

/** Cuántas cruces se graban: 4 (rápido) o 9 (permite medir la precisión). */
export type MarkCount = 4 | 9;

interface MarksUiState {
  /** El asistente de captura y detección está abierto. */
  open: boolean;
  /** Última elección de cuántas marcas, compartida por el panel y el asistente. */
  count: MarkCount;
  openDialog: () => void;
  closeDialog: () => void;
  setCount: (count: MarkCount) => void;
}

export const useMarks = create<MarksUiState>((set) => ({
  open: false,
  count: 9,
  openDialog: () => set({ open: true }),
  closeDialog: () => set({ open: false }),
  setCount: (count) => set({ count }),
}));

/** Para los comandos del menú: abre el asistente sin importar componentes. */
export function openMarksDialog(): void {
  useMarks.getState().openDialog();
}
