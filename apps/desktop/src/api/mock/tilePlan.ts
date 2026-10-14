import type { Bounds, MarkSegment, PlanArgs, PlanResult, PlannedTile, TileError } from '@/features/tiling/types';
import { MAX_TILES } from '@/features/tiling/types';

/**
 * Maqueta del plan de tiling para el modo simulado (`npm run dev` sin Tauri y las pruebas). La app
 * real llama al comando `plan_tiles`, cuyo cálculo es `tlaku_pipeline::plan_grid` (Rust, la única
 * fuente de verdad). Esta copia existe solo para que la maqueta responda igual;
 * `crates/tlaku-pipeline/tests/cross/tiling.json` la comprueba con los mismos casos que al motor
 * (`tilePlan.test.ts` y `tests/cross.rs`). Si el motor cambia, hay que cambiarla.
 */

/** Tamaño máximo de una cruz de alineación, mm. */
const MAX_MARK_MM = 6;
const EPS = 1e-9;

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

function grid(design: Bounds, size: [number, number], overlap: number): { grid: Grid } | { error: TileError } {
  const nums = [design[0][0], design[0][1], design[1][0], design[1][1], size[0], size[1], overlap];
  if (!nums.every(Number.isFinite)) return { error: { kind: 'numbers' } };
  if (design[1][0] <= design[0][0] || design[1][1] <= design[0][1]) return { error: { kind: 'empty' } };
  if (size[0] <= 0 || size[1] <= 0) return { error: { kind: 'size' } };
  if (overlap < 0) return { error: { kind: 'overlapNegative' } };
  if (overlap >= Math.min(size[0], size[1])) return { error: { kind: 'overlap', overlap, tile: size } };
  const cols = splitAxis(design[0][0], design[1][0], size[0], overlap);
  // Las filas se numeran de arriba hacia abajo y se parten desde arriba: lo que sobra queda abajo.
  const rows = splitAxis(-design[1][1], -design[0][1], size[1], overlap).map(([a, b]) => [-b, -a] as [number, number]);
  if (cols.length * rows.length > MAX_TILES) return { error: { kind: 'tooMany', count: cols.length * rows.length } };
  return { grid: { cols, rows } };
}

function marksOf(g: Grid, design: Bounds, overlap: number): MarkSegment[] {
  if (overlap <= 0 || (g.cols.length === 1 && g.rows.length === 1)) return [];
  const { cols, rows } = g;
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

/** Lo que responde la maqueta a `plan_tiles`. */
export function mockPlanTiles({ design, size, overlap, order, margin }: PlanArgs): PlanResult {
  const g = grid(design, size, overlap);
  if ('error' in g) return { ok: false, error: g.error };
  if (!Number.isFinite(margin)) return { ok: false, error: { kind: 'numbers' } };
  const { cols, rows } = g.grid;
  const tiles: PlannedTile[] = [];
  rows.forEach(([y0, y1], row) => {
    const line: PlannedTile[] = cols.map(([x0, x1], col) => ({
      spec: { rect: [[x0, y0], [x1, y1]], offset: [-x0 + margin, -y0 + margin] },
      row,
      col,
    }));
    if (order === 'serpentine' && row % 2 === 1) line.reverse();
    tiles.push(...line);
  });
  return { ok: true, plan: { tiles, cols: cols.length, rows: rows.length, colRanges: cols, rowRanges: rows, marks: marksOf(g.grid, design, overlap) } };
}
