import { describe, expect, it } from 'vitest';
import { COVERAGE_COLS, COVERAGE_ROWS, coverageGrid, coverageRatio, discardedViews, emptyRegions, rateRms } from './coverage';
import type { Vec2 } from './geometry';

const BOARD: [number, number] = [9, 6];

/** Esquinas de un tablero 9x6 que ocupa el rectángulo [x0,x1] x [y0,y1] de la imagen. */
function view(x0: number, y0: number, x1: number, y1: number): Vec2[] {
  const pts: Vec2[] = [];
  for (let j = 0; j < BOARD[1]; j++)
    for (let i = 0; i < BOARD[0]; i++) pts.push([x0 + ((x1 - x0) * i) / (BOARD[0] - 1), y0 + ((y1 - y0) * j) / (BOARD[1] - 1)]);
  return pts;
}

const SIZE: [number, number] = [600, 400]; // celdas de 100 x 100

describe('cobertura del campo', () => {
  it('sin vistas no hay nada cubierto', () => {
    const g = coverageGrid([], BOARD, SIZE);
    expect(g).toHaveLength(COVERAGE_COLS * COVERAGE_ROWS);
    expect(coverageRatio(g)).toBe(0);
    expect(emptyRegions(g)).toHaveLength(9);
  });

  it('una vista a la izquierda cubre solo las celdas cuyo centro queda dentro', () => {
    const g = coverageGrid([view(10, 10, 290, 390)], BOARD, SIZE);
    const covered = (c: number, r: number) => g[r * COVERAGE_COLS + c];
    expect(covered(0, 0)).toBe(true);
    expect(covered(2, 3)).toBe(true);
    expect(covered(3, 0)).toBe(false);
    expect(coverageRatio(g)).toBeCloseTo(12 / 24, 6);
    const empty = emptyRegions(g);
    expect(empty).toContain('tr');
    expect(empty).toContain('br');
    expect(empty).not.toContain('tl');
  });

  it('varias vistas se acumulan hasta cubrir todo', () => {
    const g = coverageGrid([view(10, 10, 290, 390), view(310, 10, 590, 390)], BOARD, SIZE);
    expect(coverageRatio(g)).toBe(1);
    expect(emptyRegions(g)).toEqual([]);
  });

  it('ignora vistas con un número de esquinas distinto', () => {
    expect(coverageRatio(coverageGrid([[[1, 1]]], BOARD, SIZE))).toBe(0);
  });

  it('el centro se menciona al final entre las zonas vacías', () => {
    const g = coverageGrid([view(10, 10, 590, 90)], BOARD, SIZE);
    expect(emptyRegions(g).at(-1)).toBe('mc');
  });
});

describe('calificación del error', () => {
  it('usa los umbrales de 0,5 / 1 / 2 px', () => {
    expect(rateRms(0.3)).toBe('excellent');
    expect(rateRms(0.5)).toBe('good');
    expect(rateRms(0.99)).toBe('good');
    expect(rateRms(1.0)).toBe('fair');
    expect(rateRms(1.99)).toBe('fair');
    expect(rateRms(2)).toBe('poor');
    expect(rateRms(Number.NaN)).toBe('poor');
  });

  it('lista las vistas que el motor no usó', () => {
    expect(discardedViews(6, [0, 1, 3, 5])).toEqual([2, 4]);
    expect(discardedViews(3, [0, 1, 2])).toEqual([]);
  });
});
