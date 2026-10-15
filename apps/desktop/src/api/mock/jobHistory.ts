import type { LaserApi } from '../api';

type Lag = <T>(value: () => T) => Promise<T>;

type Entry = Record<string, unknown>;

/** Mismos límites que el motor (`src-tauri/src/job_history.rs`). */
const MIN_MAX = 20;
const HARD_MAX = 20_000;
const DEFAULT_MAX = 1000;
const MAX_ENTRY_BYTES = 48 * 1024;

const clampMax = (max: number | null | undefined) => Math.min(HARD_MAX, Math.max(MIN_MAX, max ?? DEFAULT_MAX));

function check(entry: Entry): void {
  if (!entry || typeof entry !== 'object' || typeof entry.id !== 'string' || !entry.id || typeof entry.at !== 'number') {
    throw new Error('La entrada del historial no es válida: debe ser un objeto con «id» (texto) y «at» (número).');
  }
  if (JSON.stringify(entry).length > MAX_ENTRY_BYTES) throw new Error('La entrada del historial es demasiado grande.');
}

/**
 * Historial de trabajos en memoria (modo mock): se comporta como el archivo JSONL del motor (reemplazo por `id`,
 * rotación de las más viejas, `update` que mezcla y borra con `null`). Dura lo que la página.
 */
export function mockJobHistory(lag: Lag): Pick<LaserApi, 'jobHistoryLoad' | 'jobHistoryAppend' | 'jobHistoryUpdate' | 'jobHistoryRemove' | 'jobHistoryClear' | 'jobHistoryImport' | 'jobHistoryExport'> {
  let entries: Entry[] = [];
  const sort = () => entries.sort((a, b) => (a.at as number) - (b.at as number));
  const rotate = (max: number) => {
    if (entries.length > max) entries = entries.slice(entries.length - max);
  };
  return {
    jobHistoryLoad: ({ max }) =>
      lag(() => {
        rotate(clampMax(max));
        return structuredClone(entries);
      }),
    jobHistoryAppend: ({ entry, max }) =>
      lag(() => {
        check(entry);
        const i = entries.findIndex((e) => e.id === entry.id);
        if (i >= 0) entries[i] = structuredClone(entry);
        else entries.push(structuredClone(entry));
        sort();
        rotate(clampMax(max));
      }),
    jobHistoryUpdate: ({ id, patch }) =>
      lag(() => {
        const e = entries.find((x) => x.id === id);
        if (!e) return false;
        for (const [k, v] of Object.entries(patch)) {
          if (k === 'id' || k === 'at') continue;
          if (v === null) delete e[k];
          else e[k] = structuredClone(v);
        }
        return true;
      }),
    jobHistoryRemove: ({ ids }) =>
      lag(() => {
        const before = entries.length;
        entries = entries.filter((e) => !ids.includes(e.id as string));
        return before - entries.length;
      }),
    jobHistoryClear: () =>
      lag(() => {
        entries = [];
      }),
    jobHistoryImport: ({ entries: incoming, max }) =>
      lag(() => {
        let added = 0;
        for (const e of incoming) {
          try {
            check(e);
          } catch {
            continue;
          }
          if (entries.some((x) => x.id === e.id)) continue;
          entries.push(structuredClone(e));
          added++;
        }
        if (added > 0) {
          sort();
          rotate(clampMax(max));
        }
        return added;
      }),
    jobHistoryExport: ({ path }) =>
      lag(() => {
        if (!/\.csv$/i.test(path)) throw new Error(`${path}: el historial se exporta con la extensión .csv.`);
      }),
  };
}
