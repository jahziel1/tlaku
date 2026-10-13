import { create } from 'zustand';
import { readPref, writePref } from '@/lib/platform';

/** Cuántos archivos recientes se recuerdan. */
export const MAX_RECENT = 10;
const PREF_KEY = 'recent';

/** Lee la lista guardada; ante cualquier dato roto devuelve lo que sea válido (o nada). */
export function parseRecents(raw: string | null): string[] {
  if (!raw) return [];
  try {
    const data: unknown = JSON.parse(raw);
    if (!Array.isArray(data)) return [];
    return dedupe(data.filter((x): x is string => typeof x === 'string' && x.length > 0)).slice(0, MAX_RECENT);
  } catch {
    return [];
  }
}

function dedupe(list: string[]): string[] {
  return [...new Set(list)];
}

/** Pone `path` al frente sin repetirlo y recorta al máximo. */
export function addRecent(list: string[], path: string, max = MAX_RECENT): string[] {
  return [path, ...list.filter((p) => p !== path)].slice(0, max);
}

export function removeRecent(list: string[], path: string): string[] {
  return list.filter((p) => p !== path);
}

export function loadRecents(): string[] {
  return parseRecents(readPref(PREF_KEY));
}

function persist(list: string[]): void {
  writePref(PREF_KEY, JSON.stringify(list));
}

interface RecentState {
  paths: string[];
  add: (path: string) => void;
  remove: (path: string) => void;
  clear: () => void;
  /** Vuelve a leer el almacenamiento (pruebas). */
  reload: () => void;
}

/** Archivos recientes, guardados en localStorage (con try/catch dentro de `readPref`/`writePref`). */
export const useRecents = create<RecentState>()((set, get) => ({
  paths: loadRecents(),
  add: (path) => {
    const paths = addRecent(get().paths, path);
    persist(paths);
    set({ paths });
  },
  remove: (path) => {
    const paths = removeRecent(get().paths, path);
    persist(paths);
    set({ paths });
  },
  clear: () => {
    persist([]);
    set({ paths: [] });
  },
  reload: () => set({ paths: loadRecents() }),
}));
