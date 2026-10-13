import type { BezPath, DesignObject, Document, TraceResult, TraceSettings } from '@/api/types';
import { ensureLayerIn } from '@/lib/layers';
import { uniqueId } from '@/store/document';

/** Resolución con que se leen las imágenes del lote si el usuario no elige otra. */
export const DEFAULT_BATCH_DPI = 96;
export const BATCH_DPI = { min: 10, max: 1200 } as const;

/** Separación entre resultados al colocarlos y ancho a partir del cual se pasa a otra fila (mm). */
export const BATCH_GAP_MM = 10;
export const BATCH_ROW_MM = 400;

export interface BatchEntry {
  path: string;
  name: string;
  width_px: number;
  height_px: number;
}

/** Tamaño en mm de una imagen del lote a `dpi` puntos por pulgada. */
export function entrySize(e: Pick<BatchEntry, 'width_px' | 'height_px'>, dpi: number): [number, number] {
  const mm = 25.4 / dpi;
  return [e.width_px * mm, e.height_px * mm];
}

export interface BatchOutcome {
  path: string;
  name: string;
  size: [number, number];
  paths: BezPath[];
  warnings: string[];
}

export interface BatchFailure {
  path: string;
  name: string;
  error: string;
}

export interface BatchProgress {
  done: number;
  total: number;
  current: string | null;
}

export interface RunBatchOptions {
  entries: readonly BatchEntry[];
  settings: TraceSettings;
  dpi: number;
  trace: (args: { path: string; settings: TraceSettings; size_mm: [number, number] }) => Promise<TraceResult>;
  onProgress?: (p: BatchProgress) => void;
  /** Se consulta antes de cada archivo; con `true` el lote se detiene. */
  cancelled?: () => boolean;
  message?: (e: unknown) => string;
}

/**
 * Vectoriza uno tras otro con los mismos ajustes. Un archivo que falla no detiene al resto: queda
 * en `failures`. Con cancelación devuelve lo hecho hasta ese momento y `cancelled: true`.
 */
export async function runBatch(opts: RunBatchOptions): Promise<{ outcomes: BatchOutcome[]; failures: BatchFailure[]; cancelled: boolean }> {
  const outcomes: BatchOutcome[] = [];
  const failures: BatchFailure[] = [];
  const total = opts.entries.length;
  let cancelled = false;
  for (const [i, entry] of opts.entries.entries()) {
    if (opts.cancelled?.()) {
      cancelled = true;
      break;
    }
    opts.onProgress?.({ done: i, total, current: entry.name });
    const size = entrySize(entry, opts.dpi);
    try {
      const r = await opts.trace({ path: entry.path, settings: opts.settings, size_mm: size });
      if (r.paths.length === 0) {
        failures.push({ path: entry.path, name: entry.name, error: r.warnings[0] ?? '' });
      } else {
        outcomes.push({ path: entry.path, name: entry.name, size, paths: r.paths, warnings: r.warnings });
      }
    } catch (e) {
      failures.push({ path: entry.path, name: entry.name, error: opts.message ? opts.message(e) : String(e) });
    }
  }
  if (!cancelled) opts.onProgress?.({ done: total, total, current: null });
  return { outcomes, failures, cancelled };
}

/**
 * Objetos vectoriales de los resultados, uno por archivo, colocados en filas desde `origin` (esquina
 * inferior izquierda de la primera): de izquierda a derecha con `gap` entre ellos, y a otra fila
 * (hacia arriba) al pasar de `rowWidth`.
 */
export function layoutOutcomes(
  outcomes: readonly BatchOutcome[],
  layer: string,
  taken: Set<string>,
  origin: [number, number] = [0, 0],
  gap = BATCH_GAP_MM,
  rowWidth = BATCH_ROW_MM,
): DesignObject[] {
  const out: DesignObject[] = [];
  let x = 0;
  let y = 0;
  let rowH = 0;
  for (const o of outcomes) {
    const [w, h] = o.size;
    if (x > 0 && x + w > rowWidth) {
      x = 0;
      y += rowH + gap;
      rowH = 0;
    }
    const id = uniqueId('trace', taken);
    taken.add(id);
    out.push({
      id,
      layer,
      transform: [1, 0, 0, 1, origin[0] + x, origin[1] + y],
      kind: { type: 'vector', paths: o.paths },
      locked: false,
      hidden: false,
      group: null,
    });
    x += w + gap;
    rowH = Math.max(rowH, h);
  }
  return out;
}

/** Agrega los resultados al documento (un solo paso de deshacer). */
export function applyBatchToDoc(doc: Document, outcomes: readonly BatchOutcome[], layer: string, origin: [number, number] = [0, 0]): { doc: Document; ids: string[] } {
  if (outcomes.length === 0) return { doc, ids: [] };
  const withLayer = ensureLayerIn(doc, layer);
  const objects = layoutOutcomes(outcomes, layer, new Set(doc.objects.map((o) => o.id)), origin);
  return { doc: { ...withLayer, objects: [...withLayer.objects, ...objects] }, ids: objects.map((o) => o.id) };
}
