import { create } from 'zustand';
import { readPref, writePref } from '@/lib/platform';
import { getPref } from '@/store';

/** Cuántos archivos recientes se recuerdan por omisión (la preferencia `recentMax` lo cambia). */
export const MAX_RECENT = 10;
/** Tope de lo que se lee del disco, por encima del máximo que permite la preferencia. */
const MAX_STORED = 50;
const PREF_KEY = 'recent';

/** Lee la lista guardada; ante cualquier dato roto devuelve lo que sea válido (o nada). */
export function parseRecents(raw: string | null): string[] {
  if (!raw) return [];
  try {
    const data: unknown = JSON.parse(raw);
    if (!Array.isArray(data)) return [];
    return dedupe(data.filter((x): x is string => typeof x === 'string' && x.length > 0)).slice(0, MAX_STORED);
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
  /** Recorta la lista al nuevo máximo (al bajar la preferencia). */
  trim: (max: number) => void;
  /** Quita los que `exists` dice que ya no existen. Si no se puede comprobar, no toca nada. */
  pruneMissing: (exists: (paths: string[]) => Promise<boolean[]>) => Promise<number>;
  /** Vuelve a leer el almacenamiento (pruebas). */
  reload: () => void;
}

/** Archivos recientes, guardados en localStorage (con try/catch dentro de `readPref`/`writePref`). */
export const useRecents = create<RecentState>()((set, get) => ({
  paths: loadRecents(),
  add: (path) => {
    const paths = addRecent(get().paths, path, getPref('recentMax'));
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
  trim: (max) => {
    const paths = get().paths.slice(0, max);
    if (paths.length === get().paths.length) return;
    persist(paths);
    set({ paths });
  },
  pruneMissing: async (exists) => {
    const before = get().paths;
    let alive: boolean[];
    try {
      alive = await exists(before);
    } catch {
      return 0;
    }
    if (alive.length !== before.length) return 0;
    // La lista pudo cambiar mientras se comprobaba: se quita por ruta, no por posición.
    const gone = new Set(before.filter((_, i) => !alive[i]));
    if (gone.size === 0) return 0;
    const paths = get().paths.filter((p) => !gone.has(p));
    persist(paths);
    set({ paths });
    return gone.size;
  },
  reload: () => set({ paths: loadRecents() }),
}));
