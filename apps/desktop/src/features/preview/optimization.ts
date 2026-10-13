import type { Optimization } from '@/bindings/Optimization';
import type { I18nKey } from '@/i18n';
import { readPref, writePref } from '@/lib/platform';

export type { Optimization };

/** Opciones sí/no de la optimización, en el orden en que las muestra el diálogo. */
export type OptimizationFlag = Exclude<keyof Optimization, 'overlap_tolerance'>;

/** Valores de fábrica (los mismos que `Optimization::default()` del motor). */
export const DEFAULT_OPTIMIZATION: Optimization = {
  by_layer: true,
  by_group: false,
  by_priority: true,
  inner_first: true,
  best_start: true,
  choose_direction: true,
  remove_overlaps: false,
  overlap_tolerance: 0.01,
  hide_backlash: false,
  reduce_travel: true,
};

export interface OptimizationGroup {
  title: I18nKey;
  flags: { key: OptimizationFlag; label: I18nKey; help: I18nKey }[];
}

/** Las opciones agrupadas por lo que controlan: el orden de corte, la forma de recorrer y el resultado. */
export const OPTIMIZATION_GROUPS: OptimizationGroup[] = [
  {
    title: 'preview.opt.group.order',
    flags: [
      { key: 'by_layer', label: 'preview.opt.byLayer', help: 'preview.opt.byLayer.help' },
      { key: 'by_group', label: 'preview.opt.byGroup', help: 'preview.opt.byGroup.help' },
      { key: 'by_priority', label: 'preview.opt.byPriority', help: 'preview.opt.byPriority.help' },
      { key: 'inner_first', label: 'preview.opt.innerFirst', help: 'preview.opt.innerFirst.help' },
    ],
  },
  {
    title: 'preview.opt.group.path',
    flags: [
      { key: 'best_start', label: 'preview.opt.bestStart', help: 'preview.opt.bestStart.help' },
      { key: 'choose_direction', label: 'preview.opt.chooseDirection', help: 'preview.opt.chooseDirection.help' },
      { key: 'reduce_travel', label: 'preview.opt.reduceTravel', help: 'preview.opt.reduceTravel.help' },
    ],
  },
  {
    title: 'preview.opt.group.result',
    flags: [
      { key: 'remove_overlaps', label: 'preview.opt.removeOverlaps', help: 'preview.opt.removeOverlaps.help' },
      { key: 'hide_backlash', label: 'preview.opt.hideBacklash', help: 'preview.opt.hideBacklash.help' },
    ],
  },
];

/** Límites de la tolerancia de solapes, en mm. */
export const OVERLAP_TOLERANCE_RANGE = { min: 0.001, max: 1 } as const;

const FLAGS = Object.keys(DEFAULT_OPTIMIZATION).filter((k) => k !== 'overlap_tolerance') as OptimizationFlag[];

/** Optimización válida a partir de datos de cualquier origen: lo inválido o ausente toma el valor de fábrica. */
export function sanitizeOptimization(raw: unknown): Optimization {
  const src = raw && typeof raw === 'object' ? (raw as Record<string, unknown>) : {};
  const out: Optimization = { ...DEFAULT_OPTIMIZATION };
  for (const k of FLAGS) if (typeof src[k] === 'boolean') out[k] = src[k];
  const tol = src.overlap_tolerance;
  if (typeof tol === 'number' && Number.isFinite(tol)) {
    out.overlap_tolerance = Math.round(Math.min(OVERLAP_TOLERANCE_RANGE.max, Math.max(OVERLAP_TOLERANCE_RANGE.min, tol)) * 1000) / 1000;
  }
  return out;
}

export function sameOptimization(a: Optimization, b: Optimization): boolean {
  return a.overlap_tolerance === b.overlap_tolerance && FLAGS.every((k) => a[k] === b[k]);
}

const PREF_KEY = 'optimization';

/** La optimización que el usuario dejó la última vez (o la de fábrica). */
export function readOptimization(): Optimization {
  try {
    return sanitizeOptimization(JSON.parse(readPref(PREF_KEY) ?? 'null'));
  } catch {
    return { ...DEFAULT_OPTIMIZATION };
  }
}

export function writeOptimization(o: Optimization): void {
  writePref(PREF_KEY, JSON.stringify(o));
}
