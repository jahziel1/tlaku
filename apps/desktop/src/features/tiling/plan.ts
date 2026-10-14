import type { DesignObject, JobOptions } from '@/api/types';
import type { I18nKey } from '@/i18n';
import { polylineBez } from '@/lib/bezpath';

/**
 * Planificación del tiling (M39) en TypeScript. Es el espejo de `plan_tiles` y `tile_marks` del
 * motor (`crates/tlaku-pipeline/src/tile.rs`): la app y el simulador la necesitan sin cruzar a
 * Rust, y las pruebas de ambos lados cubren los mismos casos.
 */

export type TileSpec = NonNullable<JobOptions['tile']>;
export type Bounds = [[number, number], [number, number]];
export type TileOrder = 'rows' | 'serpentine';

/** Más tramos que esto es casi seguro un tamaño mal escrito. */
export const MAX_TILES = 400;
/** Tamaño máximo de una cruz de alineación, mm. */
export const MAX_MARK_MM = 6;
/** Prefijo del id de los objetos de las marcas de alineación. */
export const MARK_ID_PREFIX = 'tile-mark-';
/** Color de la capa de las marcas de alineación. */
export const MARK_LAYER_COLOR = '#00a0ff';
const EPS = 1e-9;

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
}

export type PlanResult = { ok: true; plan: TilePlan } | { ok: false; error: I18nKey };

/** Reparte `[lo, hi]` en tramos de a lo sumo `size` que se solapan `overlap`; el último llega a `hi`. */
function splitAxis(lo: number, hi: number, size: number, overlap: number): [number, number][] {
  const len = hi - lo;
  if (len <= size + EPS) return [[lo, hi]];
  const step = size - overlap;
  const n = Math.max(0, Math.ceil((len - size) / step - EPS)) + 1;
  return Array.from({ length: n }, (_, k) => {
    const start = lo + k * step;
    return [start, Math.min(start + size, hi)] as [number, number];
  });
}

interface Grid {
  cols: [number, number][];
  rows: [number, number][];
}

function grid(design: Bounds, size: [number, number], overlap: number): { ok: true; grid: Grid } | { ok: false; error: I18nKey } {
  const nums = [design[0][0], design[0][1], design[1][0], design[1][1], size[0], size[1], overlap];
  if (!nums.every(Number.isFinite)) return { ok: false, error: 'tiling.err.numbers' };
  if (design[1][0] <= design[0][0] || design[1][1] <= design[0][1]) return { ok: false, error: 'tiling.err.empty' };
  if (size[0] <= 0 || size[1] <= 0) return { ok: false, error: 'tiling.err.size' };
  if (overlap < 0) return { ok: false, error: 'tiling.err.overlapNegative' };
  if (overlap >= Math.min(size[0], size[1])) return { ok: false, error: 'tiling.err.overlap' };
  const cols = splitAxis(design[0][0], design[1][0], size[0], overlap);
  // Las filas se numeran de arriba hacia abajo y se parten desde arriba: lo que sobra queda abajo.
  const rows = splitAxis(-design[1][1], -design[0][1], size[1], overlap).map(([a, b]) => [-b, -a] as [number, number]);
  if (cols.length * rows.length > MAX_TILES) return { ok: false, error: 'tiling.err.tooMany' };
  return { ok: true, grid: { cols, rows } };
}

/**
 * Reparte la caja `design` en tramos de `size` (ancho, alto) con `overlap` mm de solape, en el
 * orden pedido: filas de arriba hacia abajo, y con `serpentine` cada fila impar al revés. Cada
 * tramo trae `offset = -rect.min`: su esquina inferior izquierda queda en el origen de la cama.
 */
export function planTiles(design: Bounds, size: [number, number], overlap: number, order: TileOrder): PlanResult {
  const g = grid(design, size, overlap);
  if (!g.ok) return g;
  const { cols, rows } = g.grid;
  const tiles: PlannedTile[] = [];
  rows.forEach(([y0, y1], row) => {
    const line: PlannedTile[] = cols.map(([x0, x1], col) => ({
      spec: { rect: [[x0, y0], [x1, y1]], offset: [-x0, -y0] },
      row,
      col,
    }));
    if (order === 'serpentine' && row % 2 === 1) line.reverse();
    tiles.push(...line);
  });
  return { ok: true, plan: { tiles, cols: cols.length, rows: rows.length, colRanges: cols, rowRanges: rows } };
}

/** Suma `by` al desplazamiento de todos los tramos (el margen de la cama). */
export function shiftTiles(tiles: PlannedTile[], by: [number, number]): PlannedTile[] {
  return tiles.map((p) => ({ ...p, spec: { ...p.spec, offset: [p.spec.offset[0] + by[0], p.spec.offset[1] + by[1]] } }));
}

/** Un segmento de una cruz de alineación. */
export type MarkSegment = [[number, number], [number, number]];

/**
 * Cruces de alineación en las zonas de solape (dos segmentos por cruz, en ese orden). Caen dentro
 * de dos o más tramos, así que se graban en todos ellos. Sin solape o con un solo tramo no hay
 * dónde ponerlas. Mide `min(0.8 * solape, 6)` mm.
 */
export function alignmentMarks(design: Bounds, size: [number, number], overlap: number): MarkSegment[] {
  const g = grid(design, size, overlap);
  if (!g.ok || overlap <= 0 || (g.grid.cols.length === 1 && g.grid.rows.length === 1)) return [];
  const { cols, rows } = g.grid;
  const half = Math.min(overlap * 0.8, MAX_MARK_MM) / 2;
  const m = overlap / 2;
  const xBands = cols.slice(1).map(([x0]) => x0 + m);
  const yBands = rows.slice(0, -1).map(([y0]) => y0 + m);
  const xs = [design[0][0] + m, ...xBands, design[1][0] - m];
  const ys = [design[0][1] + m, ...yBands, design[1][1] - m];
  const centers: [number, number][] = [];
  const add = (x: number, y: number) => {
    if (!centers.some(([cx, cy]) => Math.abs(cx - x) < 1e-6 && Math.abs(cy - y) < 1e-6)) centers.push([x, y]);
  };
  for (const x of xBands) for (const y of ys) add(x, y);
  for (const y of yBands) for (const x of xs) add(x, y);
  return centers.flatMap(([x, y]) => [
    [[x - half, y], [x + half, y]] as MarkSegment,
    [[x, y - half], [x, y + half]] as MarkSegment,
  ]);
}

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
