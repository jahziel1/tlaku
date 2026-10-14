import type { JobOptions } from '@/api/types';

/**
 * Tipos del plan de tiling. El cálculo es del motor (`tlaku_pipeline::plan_grid`, comando
 * `plan_tiles`); estos son los de su respuesta, con los mismos nombres de campo.
 */

export type TileSpec = NonNullable<JobOptions['tile']>;
export type Bounds = [[number, number], [number, number]];
export type TileOrder = 'rows' | 'serpentine';
/** Un segmento de una cruz de alineación. */
export type MarkSegment = [[number, number], [number, number]];

/** Más tramos que esto es casi seguro un tamaño mal escrito (el motor lo rechaza). */
export const MAX_TILES = 400;

export interface PlannedTile {
  spec: TileSpec;
  /** Fila (0 = arriba) y columna (0 = izquierda) en la cuadrícula. */
  row: number;
  col: number;
}

export interface TilePlan {
  tiles: PlannedTile[];
  cols: number;
  rows: number;
  /** Rangos `[min, max]` de cada columna (X creciente) y de cada fila (de arriba hacia abajo). */
  colRanges: [number, number][];
  rowRanges: [number, number][];
  /** Cruces de alineación en las zonas de solape (dos segmentos por cruz). */
  marks: MarkSegment[];
}

/** Por qué no se pudo repartir el diseño. */
export type TileError =
  | { kind: 'numbers' }
  | { kind: 'empty' }
  | { kind: 'size' }
  | { kind: 'overlapNegative' }
  | { kind: 'overlap'; overlap: number; tile: [number, number] }
  | { kind: 'tooMany'; count: number };

export type PlanResult = { ok: true; plan: TilePlan } | { ok: false; error: TileError };

/** Lo que se manda al motor para repartir el diseño. */
export interface PlanArgs {
  design: Bounds;
  size: [number, number];
  overlap: number;
  order: TileOrder;
  /** Margen (mm) que se suma al desplazamiento de todos los tramos. */
  margin: number;
}
