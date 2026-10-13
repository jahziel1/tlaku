/** Cobertura del campo de visión por las vistas del tablero y calificación del error. */
import { pointInPolygon, type Vec2 } from './geometry';

export const COVERAGE_COLS = 6;
export const COVERAGE_ROWS = 4;
/** Mínimo recomendado de vistas para una buena calibración. */
export const RECOMMENDED_VIEWS = 6;
/** Mínimo que acepta el motor. */
export const MIN_VIEWS = 3;

/**
 * Cuadrícula (filas de arriba abajo) de celdas del cuadro que ya cubre alguna vista. Una celda
 * está cubierta si su centro cae dentro del contorno del tablero detectado (esquinas externas),
 * que con `cols` x `rows` esquinas son la primera y última de la primera y última fila.
 */
export function coverageGrid(
  views: readonly (readonly Vec2[])[],
  board: readonly [number, number],
  size: readonly [number, number],
  cols = COVERAGE_COLS,
  rows = COVERAGE_ROWS,
): boolean[] {
  const grid = new Array<boolean>(cols * rows).fill(false);
  const [bc, br] = board;
  for (const corners of views) {
    if (corners.length !== bc * br) continue;
    const outline: Vec2[] = [corners[0]!, corners[bc - 1]!, corners[bc * br - 1]!, corners[bc * (br - 1)]!];
    for (let r = 0; r < rows; r++) {
      for (let c = 0; c < cols; c++) {
        const center: Vec2 = [((c + 0.5) * size[0]) / cols, ((r + 0.5) * size[1]) / rows];
        if (pointInPolygon(center, outline)) grid[r * cols + c] = true;
      }
    }
  }
  return grid;
}

export function coverageRatio(grid: readonly boolean[]): number {
  return grid.length === 0 ? 0 : grid.filter(Boolean).length / grid.length;
}

export type Region = 'tl' | 'tc' | 'tr' | 'ml' | 'mc' | 'mr' | 'bl' | 'bc' | 'br';

/** Zonas (tercios del cuadro) donde no hay ninguna celda cubierta, de más a menos descubierta. */
export function emptyRegions(grid: readonly boolean[], cols = COVERAGE_COLS, rows = COVERAGE_ROWS): Region[] {
  const names: Region[][] = [['tl', 'tc', 'tr'], ['ml', 'mc', 'mr'], ['bl', 'bc', 'br']];
  const total = new Map<Region, number>();
  const covered = new Map<Region, number>();
  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      const reg = names[Math.min(2, Math.floor((r * 3) / rows))]![Math.min(2, Math.floor((c * 3) / cols))]!;
      total.set(reg, (total.get(reg) ?? 0) + 1);
      if (grid[r * cols + c]) covered.set(reg, (covered.get(reg) ?? 0) + 1);
    }
  }
  return [...total.keys()].filter((reg) => !covered.get(reg)).sort((a, b) => (a === 'mc' ? 1 : b === 'mc' ? -1 : 0));
}

export type RmsGrade = 'excellent' | 'good' | 'fair' | 'poor';

/** Calificación del error de reproyección (rms, px). */
export function rateRms(rms: number): RmsGrade {
  if (!Number.isFinite(rms) || rms >= 2) return 'poor';
  if (rms < 0.5) return 'excellent';
  if (rms < 1) return 'good';
  return 'fair';
}

/** Vistas que el motor descartó: índices de entrada que no están en `used`. */
export function discardedViews(total: number, used: readonly number[]): number[] {
  const set = new Set(used);
  return Array.from({ length: total }, (_, i) => i).filter((i) => !set.has(i));
}
