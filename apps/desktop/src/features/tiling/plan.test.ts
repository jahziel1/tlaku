import { describe, expect, it } from 'vitest';
import { alignmentMarks, defaultTileSize, exceedsBed, feedBetween, isSingleDirection, planTiles, shiftTiles, type Bounds, type TilePlan, type TileSpec } from './plan';

const D: Bounds = [[0, 0], [100, 60]];

const plan = (design: Bounds, size: [number, number], overlap: number, order: 'rows' | 'serpentine' = 'rows'): TilePlan => {
  const r = planTiles(design, size, overlap, order);
  if (!r.ok) throw new Error(r.error);
  return r.plan;
};

const inRect = (r: TileSpec['rect'], x: number, y: number) => x >= r[0][0] - 1e-9 && x <= r[1][0] + 1e-9 && y >= r[0][1] - 1e-9 && y <= r[1][1] + 1e-9;

describe('planTiles', () => {
  it('un diseño que cabe es un solo tramo, llevado al origen', () => {
    const p = plan([[10, 20], [60, 50]], [100, 100], 5);
    expect(p.tiles).toHaveLength(1);
    expect(p.tiles[0]!.spec).toEqual({ rect: [[10, 20], [60, 50]], offset: [-10, -20] });
  });

  it('un rectángulo de 100 x 60 en tramos de 55 x 35 con 10 mm de solape son 4 tramos', () => {
    const p = plan(D, [55, 35], 10);
    expect(p.tiles).toHaveLength(4);
    expect([p.cols, p.rows]).toEqual([2, 2]);
    // Filas de arriba hacia abajo, cada fila de izquierda a derecha.
    expect(p.tiles.map((t) => t.spec.rect)).toEqual([
      [[0, 25], [55, 60]],
      [[45, 25], [100, 60]],
      [[0, 0], [55, 35]],
      [[45, 0], [100, 35]],
    ]);
    expect(p.tiles.map((t) => [t.row, t.col])).toEqual([[0, 0], [0, 1], [1, 0], [1, 1]]);
    for (const { spec } of p.tiles) expect(spec.offset).toEqual([-spec.rect[0][0], -spec.rect[0][1]]);
    // El solape entre columnas y entre filas es el pedido.
    expect(p.tiles[0]!.spec.rect[1][0] - p.tiles[1]!.spec.rect[0][0]).toBeCloseTo(10);
    expect(p.tiles[0]!.spec.rect[0][1] - p.tiles[2]!.spec.rect[1][1]).toBeCloseTo(-10);
  });

  it('ningún tramo pasa del tamaño pedido y todos juntos cubren el diseño', () => {
    const d: Bounds = [[3, -7], [321, 190]];
    const p = plan(d, [120, 80], 4);
    for (const { spec } of p.tiles) {
      expect(spec.rect[1][0] - spec.rect[0][0]).toBeLessThanOrEqual(120 + 1e-9);
      expect(spec.rect[1][1] - spec.rect[0][1]).toBeLessThanOrEqual(80 + 1e-9);
    }
    for (let x = d[0][0]; x <= d[1][0]; x += 3.7) {
      for (let y = d[0][1]; y <= d[1][1]; y += 3.7) {
        expect(p.tiles.some((t) => inRect(t.spec.rect, x, y)), `(${x}, ${y})`).toBe(true);
      }
    }
  });

  it('la serpentina invierte una de cada dos filas', () => {
    const rows = plan(D, [55, 35], 10, 'rows').tiles;
    const snake = plan(D, [55, 35], 10, 'serpentine').tiles;
    expect(snake.map((t) => t.spec)).toEqual([rows[0], rows[1], rows[3], rows[2]].map((t) => t!.spec));
  });

  it('rechaza valores imposibles con la clave del motivo', () => {
    const err = (size: [number, number], overlap: number, d: Bounds = [[0, 0], [100, 100]]) => {
      const r = planTiles(d, size, overlap, 'rows');
      return r.ok ? null : r.error;
    };
    expect(err([0, 50], 0)).toBe('tiling.err.size');
    expect(err([50, 50], 50)).toBe('tiling.err.overlap');
    expect(err([50, 50], -1)).toBe('tiling.err.overlapNegative');
    expect(err([Number.NaN, 50], 0)).toBe('tiling.err.numbers');
    expect(err([50, 50], 0, [[0, 0], [0, 5]])).toBe('tiling.err.empty');
    expect(err([0.5, 0.5], 0.1)).toBe('tiling.err.tooMany');
  });

  it('shiftTiles suma el margen al desplazamiento', () => {
    const p = plan(D, [55, 35], 10);
    const shifted = shiftTiles(p.tiles, [5, 5]);
    expect(shifted[1]!.spec.offset).toEqual([-45 + 5, -25 + 5]);
    expect(p.tiles[1]!.spec.offset).toEqual([-45, -25]);
  });
});

describe('alignmentMarks', () => {
  it('cada cruz cae entera dentro de dos o más tramos', () => {
    const p = plan(D, [55, 35], 10);
    const marks = alignmentMarks(D, [55, 35], 10);
    expect(marks.length).toBeGreaterThanOrEqual(4);
    expect(marks.length % 2).toBe(0);
    for (let i = 0; i < marks.length; i += 2) {
      const pts = marks.slice(i, i + 2).flat();
      const n = p.tiles.filter((t) => pts.every(([x, y]) => inRect(t.spec.rect, x, y))).length;
      expect(n, `cruz ${i / 2 + 1}`).toBeGreaterThanOrEqual(2);
    }
  });

  it('mide 0,8 veces el solape con un tope de 6 mm', () => {
    const [h, v] = alignmentMarks(D, [55, 35], 3);
    expect(Math.abs(h![1][0] - h![0][0])).toBeCloseTo(2.4);
    expect(Math.abs(v![1][1] - v![0][1])).toBeCloseTo(2.4);
    const [big] = alignmentMarks(D, [55, 35], 20);
    expect(Math.abs(big![1][0] - big![0][0])).toBeCloseTo(6);
  });

  it('sin solape o con un solo tramo no hay marcas', () => {
    expect(alignmentMarks(D, [55, 35], 0)).toEqual([]);
    expect(alignmentMarks(D, [200, 200], 5)).toEqual([]);
  });
});

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
