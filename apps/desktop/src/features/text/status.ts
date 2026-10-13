import { create } from 'zustand';

/** Estado de la generación del contorno de un texto (para mostrarlo en el panel). */
export interface TextStatus {
  /** Hay un contorno por generar (en espera del debounce o en camino). */
  busy: boolean;
  /** Advertencias del motor (fuente sin cursiva, carácter sin glifo, no cabe...). */
  warnings: string[];
  /** Mensaje de error del motor o de la interfaz; el contorno anterior se conserva. */
  error: string | null;
}

export const IDLE_STATUS: TextStatus = { busy: false, warnings: [], error: null };

interface StatusStore {
  byId: Record<string, TextStatus>;
  set: (id: string, patch: Partial<TextStatus>) => void;
  clear: (id: string) => void;
  reset: () => void;
}

/** Estado por id de texto. Vive fuera del documento: no es parte del proyecto ni del historial. */
export const useTextStatus = create<StatusStore>()((set) => ({
  byId: {},
  set: (id, patch) =>
    set((s) => {
      const prev = s.byId[id] ?? IDLE_STATUS;
      const next = { ...prev, ...patch };
      const same =
        next.busy === prev.busy &&
        next.error === prev.error &&
        next.warnings.length === prev.warnings.length &&
        next.warnings.every((w, i) => w === prev.warnings[i]);
      return same ? s : { byId: { ...s.byId, [id]: next } };
    }),
  clear: (id) =>
    set((s) => {
      if (!(id in s.byId)) return s;
      const { [id]: _gone, ...rest } = s.byId;
      return { byId: rest };
    }),
  reset: () => set({ byId: {} }),
}));

export const selectStatus = (id: string) => (s: StatusStore): TextStatus => s.byId[id] ?? IDLE_STATUS;
