import type { DesignObject } from '@/api/types';
import { polylineBez } from '@/lib/bezpath';
import type { Bounds, MarkSegment, TilePlan, TileSpec } from './types';

/**
 * Ayudas del tiling (M39) que no son el plan: el plan (reparto en tramos, solape, orden y cruces de
 * alineación) lo calcula el motor (`plan_tiles`, ver `types.ts`); aquí viven la comprobación contra
 * la cama, el tamaño por defecto, el avance del alimentador y los objetos de las cruces.
 */

export type { Bounds, MarkSegment, PlannedTile, TileOrder, TilePlan, TileSpec } from './types';
export { MAX_TILES } from './types';

/** Prefijo del id de los objetos de las marcas de alineación (el mismo que el motor). */
export const MARK_ID_PREFIX = 'tile-mark-';
/** Color de la capa de las marcas de alineación. */
export const MARK_LAYER_COLOR = '#00a0ff';
const EPS = 1e-9;

/** El diseño es más grande que la cama en alguno de los dos ejes. */
export function exceedsBed(design: Bounds, bed: readonly [number, number]): boolean {
  return design[1][0] - design[0][0] > bed[0] + EPS || design[1][1] - design[0][1] > bed[1] + EPS;
}

/** Tamaño de tramo por defecto: la cama menos un margen por lado (el overscan necesita aire). */
export function defaultTileSize(bed: readonly [number, number], margin: number): [number, number] {
  return [Math.max(1, bed[0] - 2 * margin), Math.max(1, bed[1] - 2 * margin)];
}

/**
 * Cuánto avanza el material para pasar del tramo `a` al `b` cuando están en la misma columna
 * (con alimentador): la diferencia entre sus bordes inferiores. Para los tramos completos es el
 * alto menos el solape. `null` si no están en la misma columna.
 */
export function feedBetween(a: TileSpec, b: TileSpec): number | null {
  if (Math.abs(a.rect[0][0] - b.rect[0][0]) > EPS) return null;
  const mm = a.rect[0][1] - b.rect[0][1];
  return Math.abs(mm) > EPS ? Math.abs(mm) : null;
}

/** ¿Los tramos van todos en una sola dirección (una columna)? Condición para usar el alimentador. */
export function isSingleDirection(plan: TilePlan): boolean {
  return plan.cols === 1 && plan.rows > 1;
}

// ---- marcas como objetos del documento -------------------------------------------------------

/** Objetos vectoriales de las cruces (uno por cruz) para la capa de marcas. */
export function markObjects(segments: MarkSegment[], layer: string = MARK_LAYER_COLOR): DesignObject[] {
  const out: DesignObject[] = [];
  for (let i = 0; i < segments.length; i += 2) {
    const cross = segments.slice(i, i + 2);
    out.push({
      id: `${MARK_ID_PREFIX}${i / 2 + 1}`,
      layer,
      transform: [1, 0, 0, 1, 0, 0],
      kind: {
        type: 'vector',
        paths: cross.map(([a, b]) => polylineBez([a, b], false)),
      },
      locked: false,
      hidden: false,
      group: null,
    });
  }
  return out;
}

export function boundsSize(b: Bounds): [number, number] {
  return [b[1][0] - b[0][0], b[1][1] - b[0][1]];
}
