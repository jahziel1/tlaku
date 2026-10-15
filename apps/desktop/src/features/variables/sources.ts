import type { CsvTable, DataSource, ShortRows } from '@/api/types';
import { cleanSourceName } from './defaults';

/**
 * Operaciones puras sobre la lista de fuentes de datos del proyecto. La primera es la principal:
 * manda el número de filas y la fila actual; las demás se leen con el mismo índice.
 */

function stem(path: string): string {
  const base = path.split(/[\\/]/).pop() ?? path;
  const dot = base.lastIndexOf('.');
  return dot > 0 ? base.slice(0, dot) : base;
}

/** Un nombre válido y libre (sin distinguir mayúsculas) a partir de `wanted`. */
export function freeSourceName(sources: readonly DataSource[], wanted: string, ignoreIndex = -1): string {
  const taken = new Set(sources.filter((_, i) => i !== ignoreIndex).map((s) => s.name.toLowerCase()));
  const base = cleanSourceName(wanted) || 'datos';
  let name = base;
  for (let n = 2; taken.has(name.toLowerCase()); n++) name = `${base}${n}`;
  return name;
}

/** Agrega un CSV al final con el nombre de su archivo (la primera fuente queda como principal). */
export function addSource(sources: readonly DataSource[], path: string): DataSource[] {
  return [...sources, { name: freeSourceName(sources, stem(path)), path, on_short: 'repeat' }];
}

/** Cambia el archivo de una fuente (el nombre se conserva). */
export function setSourcePath(sources: readonly DataSource[], index: number, path: string): DataSource[] {
  return sources.map((s, i) => (i === index ? { ...s, path } : s));
}

/** Quita una fuente. */
export function removeSource(sources: readonly DataSource[], index: number): DataSource[] {
  return sources.filter((_, i) => i !== index);
}

/** Pone la fuente `index` como principal (primera); las demás conservan su orden. */
export function makeMain(sources: readonly DataSource[], index: number): DataSource[] {
  if (index <= 0 || index >= sources.length) return [...sources];
  return [sources[index]!, ...sources.filter((_, i) => i !== index)];
}

/** Cambia el nombre de una fuente (válido y sin repetir). Las plantillas que la nombran se reescriben con `renameInTemplate`. */
export function renameSource(sources: readonly DataSource[], index: number, wanted: string): DataSource[] {
  const name = freeSourceName(sources, wanted, index);
  return sources.map((s, i) => (i === index ? { ...s, name } : s));
}

/** Qué hacer cuando la fuente tiene menos filas que la principal. */
export function setOnShort(sources: readonly DataSource[], index: number, value: ShortRows): DataSource[] {
  return sources.map((s, i) => (i === index ? { ...s, on_short: value } : s));
}

/** Cómo se llevan las filas de una fuente secundaria con las de la principal. */
export type RowFit =
  | { kind: 'same' } //  mismo número de filas
  | { kind: 'repeat'; rows: number; main: number } //  más corta: se repite desde el inicio
  | { kind: 'block'; rows: number; main: number } //  más corta: el envío se bloquea al pasar de su última fila
  | { kind: 'extra'; rows: number; main: number } //  más larga: las filas que sobran no se usan
  | { kind: 'empty' }; //  sin filas

/** Avisos sobre el número de filas de las fuentes secundarias frente a la principal. */
export function rowFits(sources: readonly DataSource[], tables: readonly (CsvTable | null)[]): (RowFit | null)[] {
  const main = tables[0]?.rows.length ?? null;
  return sources.map((s, i) => {
    if (i === 0) return null;
    const table = tables[i];
    if (!table || main === null) return null;
    const rows = table.rows.length;
    if (rows === 0) return { kind: 'empty' };
    if (rows === main) return { kind: 'same' };
    if (rows > main) return { kind: 'extra', rows, main };
    return { kind: s.on_short === 'block' ? 'block' : 'repeat', rows, main };
  });
}

/** Reescribe en una plantilla las referencias `{csv.viejo:...}` / `{fila.viejo}` al nombre nuevo. */
export function renameInTemplate(template: string, from: string, to: string): string {
  const f = from.toLowerCase();
  return template.replace(/\{(csv|fila|row)\.([^:@}]*)/gi, (m, base: string, name: string) => (name.trim().toLowerCase() === f ? `{${base}.${to}` : m));
}
