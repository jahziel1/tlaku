import type { JobLogEntry, JobResult } from './jobLog';

/** Filtros del historial de trabajos. Los vacíos no filtran. */
export interface JobFilter {
  /** Texto libre: documento, pestaña, máquina, material, capas, mensaje y notas. */
  text: string;
  machine: string;
  material: string;
  /** `''` = todos; `interrupted` incluye también los que dejaron punto de control para continuar. */
  result: '' | JobResult;
  /** Fechas locales `AAAA-MM-DD`, ambas incluidas. */
  from: string;
  to: string;
}

export const EMPTY_FILTER: JobFilter = { text: '', machine: '', material: '', result: '', from: '', to: '' };

export const isFilterEmpty = (f: JobFilter): boolean => f.text.trim() === '' && !f.machine && !f.material && !f.result && !f.from && !f.to;

/** Minúsculas y sin acentos, para que «acrilico» encuentre «Acrílico». */
export function fold(s: string): string {
  return s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
}

/** Inicio (o fin) del día local de `AAAA-MM-DD`, en ms; `null` si el texto no es una fecha. */
export function dayBound(day: string, end: boolean): number | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(day);
  if (!m) return null;
  const d = new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]), end ? 23 : 0, end ? 59 : 0, end ? 59 : 0, end ? 999 : 0);
  return Number.isNaN(d.getTime()) ? null : d.getTime();
}

/** Materiales distintos que aparecen en el historial (cada capa puede traer el suyo). */
export function materialsOf(e: JobLogEntry): string[] {
  const names = new Set<string>();
  for (const l of e.layers ?? []) if (l.material) names.add(l.material);
  if (e.material) for (const m of e.material.split(', ')) if (m) names.add(m);
  return [...names];
}

function haystack(e: JobLogEntry): string {
  return fold(
    [e.document ?? '', e.documentPath ?? '', e.tab ?? '', e.machine, e.message, e.notes ?? '', ...materialsOf(e), ...(e.layers ?? []).map((l) => `${l.name} ${l.mode}`)].join('\n'),
  );
}

export function matchesFilter(e: JobLogEntry, f: JobFilter): boolean {
  if (f.machine && e.machine !== f.machine) return false;
  if (f.material && !materialsOf(e).includes(f.material)) return false;
  if (f.result && !(e.result === f.result || (f.result === 'interrupted' && e.interrupted !== undefined))) return false;
  const from = f.from ? dayBound(f.from, false) : null;
  const to = f.to ? dayBound(f.to, true) : null;
  if (from !== null && e.at < from) return false;
  if (to !== null && e.at > to) return false;
  const words = fold(f.text).split(/\s+/).filter(Boolean);
  if (words.length > 0) {
    const hay = haystack(e);
    if (!words.every((w) => hay.includes(w))) return false;
  }
  return true;
}

/** Las entradas que cumplen todos los filtros, en el mismo orden. */
export function filterJobs(entries: readonly JobLogEntry[], f: JobFilter): JobLogEntry[] {
  return isFilterEmpty(f) ? [...entries] : entries.filter((e) => matchesFilter(e, f));
}

/** Valores distintos (ordenados) para llenar las listas de máquina y material. */
export function distinctMachines(entries: readonly JobLogEntry[]): string[] {
  return [...new Set(entries.map((e) => e.machine))].sort((a, b) => a.localeCompare(b));
}

export function distinctMaterials(entries: readonly JobLogEntry[]): string[] {
  return [...new Set(entries.flatMap(materialsOf))].sort((a, b) => a.localeCompare(b));
}
