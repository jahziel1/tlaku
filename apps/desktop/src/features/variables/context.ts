import { create } from 'zustand';
import type { CsvTable, Document } from '@/api/types';
import { getLanguage } from '@/i18n';
import { variableTextOf } from './defaults';
import { makeContext, type VarContext } from './resolve';

/** CSV del documento ya leído (o el error al leerlo). */
export interface CsvState {
  path: string | null;
  table: CsvTable | null;
  error: string | null;
  loading: boolean;
}

export const EMPTY_CSV: CsvState = { path: null, table: null, error: null, loading: false };

interface VariablesUi {
  /** Diálogo "Texto variable" abierto. */
  open: boolean;
  /** En el lienzo se ve la plantilla (`{serie}`) en lugar de los valores actuales. */
  showTemplate: boolean;
  /** Hora con la que se ven fecha y hora en el lienzo (se refresca cada minuto). */
  clock: number;
  /** La fuente principal (la primera del documento). */
  csv: CsvState;
  /** Las demás fuentes, por nombre en minúsculas. */
  extra: Record<string, CsvState>;
  show: () => void;
  close: () => void;
  setShowTemplate: (v: boolean) => void;
  setClock: (ms: number) => void;
  setCsv: (csv: CsvState) => void;
  /** Reemplaza el estado de las fuentes secundarias. */
  setExtra: (extra: Record<string, CsvState>) => void;
}

/** Estado de interfaz del texto variable: no forma parte del documento ni del historial. */
export const useVariablesUi = create<VariablesUi>()((set) => ({
  open: false,
  showTemplate: false,
  clock: Date.now(),
  csv: EMPTY_CSV,
  extra: {},
  show: () => set({ open: true }),
  close: () => set({ open: false }),
  setShowTemplate: (showTemplate) => set({ showTemplate }),
  setClock: (clock) => set({ clock }),
  setCsv: (csv) => set({ csv }),
  setExtra: (extra) => set({ extra }),
}));

/** Tablas ya leídas de las fuentes secundarias del documento (solo las que coinciden con su ruta). */
export function extraTables(doc: Pick<Document, 'variable_text'>): Record<string, CsvTable | null> {
  const ui = useVariablesUi.getState();
  const out: Record<string, CsvTable | null> = {};
  variableTextOf(doc).sources.slice(1).forEach((s) => {
    const st = ui.extra[s.name.toLowerCase()];
    out[s.name.toLowerCase()] = st && st.path === s.path ? st.table : null;
  });
  return out;
}

/** Contexto con el que se ve el documento en el lienzo: valores actuales y los CSV ya cargados. */
export function displayContext(doc: Document): VarContext {
  const ui = useVariablesUi.getState();
  const path = variableTextOf(doc).csv_path;
  const table = path !== null && ui.csv.path === path ? ui.csv.table : null;
  return makeContext(doc.variable_text, table, new Date(ui.clock), getLanguage(), extraTables(doc));
}
