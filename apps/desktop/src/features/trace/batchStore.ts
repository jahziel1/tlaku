import { create } from 'zustand';
import type { TraceSettings } from '@/api/types';
import { DEFAULT_BATCH_DPI, type BatchEntry, type BatchFailure, type BatchProgress } from './batch';
import { DEFAULT_TRACE_SETTINGS } from './settings';

interface BatchDialogState {
  open: boolean;
  entries: BatchEntry[];
  /** Archivo cuya vista previa se muestra. */
  selected: string | null;
  settings: TraceSettings;
  dpi: number;
  /** `null` mientras no corre; si no, el avance. */
  progress: BatchProgress | null;
  failures: BatchFailure[];
  show: (settings: TraceSettings) => void;
  close: () => void;
  addEntries: (list: BatchEntry[]) => void;
  removeEntry: (path: string) => void;
  select: (path: string | null) => void;
  patchSettings: (patch: Partial<TraceSettings>) => void;
  resetSettings: () => void;
  setDpi: (dpi: number) => void;
  setProgress: (p: BatchProgress | null) => void;
  setFailures: (f: BatchFailure[]) => void;
}

/** Estado de interfaz del diálogo "Vectorizar varios archivos" (no es parte del documento). */
export const useBatchDialog = create<BatchDialogState>()((set) => ({
  open: false,
  entries: [],
  selected: null,
  settings: DEFAULT_TRACE_SETTINGS,
  dpi: DEFAULT_BATCH_DPI,
  progress: null,
  failures: [],
  // Parte de los ajustes de la última vectorización individual.
  show: (settings) => set({ open: true, settings, progress: null, failures: [] }),
  close: () => set({ open: false, progress: null }),
  addEntries: (list) =>
    set((s) => {
      const known = new Set(s.entries.map((e) => e.path));
      const fresh = list.filter((e) => !known.has(e.path));
      const entries = [...s.entries, ...fresh];
      return { entries, selected: s.selected ?? entries[0]?.path ?? null };
    }),
  removeEntry: (path) =>
    set((s) => {
      const entries = s.entries.filter((e) => e.path !== path);
      return { entries, selected: s.selected === path ? (entries[0]?.path ?? null) : s.selected };
    }),
  select: (selected) => set({ selected }),
  patchSettings: (patch) => set((s) => ({ settings: { ...s.settings, ...patch } })),
  resetSettings: () => set({ settings: DEFAULT_TRACE_SETTINGS }),
  setDpi: (dpi) => set({ dpi }),
  setProgress: (progress) => set({ progress }),
  setFailures: (failures) => set({ failures }),
}));
