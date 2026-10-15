import type { Point } from '@/api/types';
import { entryMaxRate, type CatalogEntry } from './catalog';

/** Ajustes de GRBL por número: `{ "$130": "400.000" }` -> `{ 130: 400 }`. Ignora lo que no es un número. */
export function numericSettings(raw: Record<string, string> | null): Record<number, number> {
  const out: Record<number, number> = {};
  for (const [key, value] of Object.entries(raw ?? {})) {
    const m = /^\$(\d+)$/.exec(key.trim());
    const n = Number.parseFloat(value);
    if (m && Number.isFinite(n)) out[Number(m[1])] = n;
  }
  return out;
}

/** Datos que se comparan entre el catálogo y lo que la máquina contestó a `$$`. */
export type CompareField = 's_max' | 'bed' | 'max_rate' | 'accel';

export type CompareStatus =
  /** Catálogo y máquina dicen lo mismo. */
  | 'match'
  /** Los dos tienen valor y es distinto. */
  | 'differs'
  /** El catálogo no sabe este dato: el de la máquina es el único. */
  | 'catalogEmpty'
  /** La máquina no contestó este ajuste. */
  | 'unread';

export interface CompareRow {
  field: CompareField;
  /** Ajustes de GRBL de donde sale el valor, por ejemplo `$130/$131`. */
  codes: string;
  catalog: number | Point | null;
  machine: number | Point | null;
  status: CompareStatus;
}

/** `$32`: 1 = modo láser; 0 = desactivado (M4 no sigue a la velocidad); otro (grblHAL: 2 = torno). */
export type LaserModeState = 'on' | 'off' | 'other' | 'unknown';

export interface Comparison {
  rows: CompareRow[];
  laserMode: LaserModeState;
  /** La máquina no devolvió ninguno de los ajustes que se comparan. */
  nothingRead: boolean;
}

const EPS = 0.01;
const same = (a: number, b: number) => Math.abs(a - b) < EPS;
const samePoint = (a: Point, b: Point) => same(a[0], b[0]) && same(a[1], b[1]);

function pair(s: Record<number, number>, x: number, y: number): Point | null {
  const vx = s[x];
  const vy = s[y];
  return vx !== undefined && vy !== undefined && vx > 0 && vy > 0 ? [vx, vy] : null;
}

function status(catalog: number | Point | null, machine: number | Point | null): CompareStatus {
  if (machine === null) return 'unread';
  if (catalog === null) return 'catalogEmpty';
  if (typeof catalog === 'number' && typeof machine === 'number') return same(catalog, machine) ? 'match' : 'differs';
  return samePoint(catalog as Point, machine as Point) ? 'match' : 'differs';
}

/**
 * Compara los datos del catálogo con los ajustes leídos de la máquina: `$30` con la `S` máxima,
 * `$130/$131` con la cama, `$110/$111` con la velocidad máxima y `$120/$121` con la aceleración.
 * Sin entrada (modelo "otro") todo el catálogo está vacío. `$32` se evalúa aparte.
 */
export function compareWithMachine(entry: CatalogEntry | null, raw: Record<string, string> | null): Comparison {
  const s = numericSettings(raw);
  const sMax = s[30] !== undefined && s[30] > 0 ? s[30] : null;
  const maxRate = pair(s, 110, 111);
  const accel = pair(s, 120, 121);
  const rate = entry ? entryMaxRate(entry) : null;
  const acc = entry?.accel?.value;
  const rows: CompareRow[] = [
    { field: 's_max', codes: '$30', catalog: entry?.sMax?.value ?? null, machine: sMax, status: 'unread' },
    { field: 'bed', codes: '$130/$131', catalog: entry?.bed?.value ?? null, machine: pair(s, 130, 131), status: 'unread' },
    { field: 'max_rate', codes: '$110/$111', catalog: rate, machine: maxRate, status: 'unread' },
    { field: 'accel', codes: '$120/$121', catalog: acc === undefined ? null : [acc, acc], machine: accel, status: 'unread' },
  ];
  for (const r of rows) r.status = status(r.catalog, r.machine);
  const laser = s[32];
  return {
    rows,
    laserMode: laser === undefined ? 'unknown' : laser === 1 ? 'on' : laser === 0 ? 'off' : 'other',
    nothingRead: rows.every((r) => r.machine === null) && laser === undefined,
  };
}

/**
 * Por defecto se propone lo que dice la máquina cuando difiere del catálogo o el catálogo no lo
 * sabe: lo medido en la máquina manda sobre lo anunciado. El usuario puede dejar el del catálogo.
 */
export function defaultMachineChoices(c: Comparison): Set<CompareField> {
  return new Set(c.rows.filter((r) => r.status === 'differs' || r.status === 'catalogEmpty').map((r) => r.field));
}
