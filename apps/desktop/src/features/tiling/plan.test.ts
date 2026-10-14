import { describe, expect, it } from 'vitest';
import { mockPlanTiles } from '@/api/mock/tilePlan';
import { defaultTileSize, exceedsBed, feedBetween, isSingleDirection, type Bounds, type TilePlan } from './plan';

const D: Bounds = [[0, 0], [100, 60]];

const plan = (design: Bounds, size: [number, number], overlap: number): TilePlan => {
  const r = mockPlanTiles({ design, size, overlap, order: 'rows', margin: 0 });
  if (!r.ok) throw new Error(r.error.kind);
  return r.plan;
};

describe('ayudas', () => {
  it('detecta que el diseño no cabe en la cama', () => {
    expect(exceedsBed([[0, 0], [500, 100]], [400, 400])).toBe(true);
    expect(exceedsBed([[0, 0], [400, 400]], [400, 400])).toBe(false);
  });

  it('tamaño de tramo por defecto: la cama menos el margen por lado', () => {
    expect(defaultTileSize([400, 300], 5)).toEqual([390, 290]);
  });

  it('una sola dirección = una columna con varias filas', () => {
    expect(isSingleDirection(plan([[0, 0], [50, 200]], [60, 80], 10))).toBe(true);
    expect(isSingleDirection(plan(D, [55, 35], 10))).toBe(false);
    expect(isSingleDirection(plan([[0, 0], [50, 50]], [60, 80], 10))).toBe(false);
  });

  it('el avance entre tramos de una columna es el alto menos el solape', () => {
    const p = plan([[0, 0], [50, 200]], [60, 80], 10);
    expect(feedBetween(p.tiles[0]!.spec, p.tiles[1]!.spec)).toBeCloseTo(70);
    // El último tramo, más corto, avanza solo lo que falta.
    const last = p.tiles.length - 1;
    expect(feedBetween(p.tiles[last - 1]!.spec, p.tiles[last]!.spec)).toBeLessThanOrEqual(70 + 1e-9);
    // Columnas distintas: no hay avance posible.
    const grid = plan(D, [55, 35], 10);
    expect(feedBetween(grid.tiles[0]!.spec, grid.tiles[1]!.spec)).toBeNull();
  });
});
