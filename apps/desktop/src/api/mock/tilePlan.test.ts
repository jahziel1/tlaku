import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import type { Bounds, MarkSegment, TileOrder, TilePlan, TileSpec } from '@/features/tiling/types';
import { mockPlanTiles } from './tilePlan';

const D: Bounds = [[0, 0], [100, 60]];

const plan = (design: Bounds, size: [number, number], overlap: number, order: TileOrder = 'rows'): TilePlan => {
  const r = mockPlanTiles({ design, size, overlap, order, margin: 0 });
  if (!r.ok) throw new Error(r.error.kind);
  return r.plan;
};
const marksOf = (design: Bounds, size: [number, number], overlap: number): MarkSegment[] => plan(design, size, overlap).marks;

const inRect = (r: TileSpec['rect'], x: number, y: number) => x >= r[0][0] - 1e-9 && x <= r[1][0] + 1e-9 && y >= r[0][1] - 1e-9 && y <= r[1][1] + 1e-9;

describe('maqueta de plan_tiles', () => {
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
    for (const { spec } of p.tiles) {
      expect(spec.offset[0]).toBeCloseTo(-spec.rect[0][0]);
      expect(spec.offset[1]).toBeCloseTo(-spec.rect[0][1]);
    }
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
      const r = mockPlanTiles({ design: d, size, overlap, order: 'rows', margin: 0 });
      return r.ok ? null : r.error.kind;
    };
    expect(err([0, 50], 0)).toBe('size');
    expect(err([50, 50], 50)).toBe('overlap');
    expect(err([50, 50], -1)).toBe('overlapNegative');
    expect(err([Number.NaN, 50], 0)).toBe('numbers');
    expect(err([50, 50], 0, [[0, 0], [0, 5]])).toBe('empty');
    expect(err([0.5, 0.5], 0.1)).toBe('tooMany');
  });

  it('el margen se suma al desplazamiento de todos los tramos', () => {
    const base = plan(D, [55, 35], 10);
    const r = mockPlanTiles({ design: D, size: [55, 35], overlap: 10, order: 'rows', margin: 5 });
    if (!r.ok) throw new Error('falló');
    expect(r.plan.tiles[1]!.spec.offset).toEqual([-45 + 5, -25 + 5]);
    expect(base.tiles[1]!.spec.offset).toEqual([-45, -25]);
  });
});

describe('maqueta: cruces de alineación', () => {
  it('cada cruz cae entera dentro de dos o más tramos', () => {
    const p = plan(D, [55, 35], 10);
    const marks = marksOf(D, [55, 35], 10);
    expect(marks.length).toBeGreaterThanOrEqual(4);
    expect(marks.length % 2).toBe(0);
    for (let i = 0; i < marks.length; i += 2) {
      const pts = marks.slice(i, i + 2).flat();
      const n = p.tiles.filter((t) => pts.every(([x, y]) => inRect(t.spec.rect, x, y))).length;
      expect(n, `cruz ${i / 2 + 1}`).toBeGreaterThanOrEqual(2);
    }
  });

  it('mide 0,8 veces el solape con un tope de 6 mm', () => {
    const [h, v] = marksOf(D, [55, 35], 3);
    expect(Math.abs(h![1][0] - h![0][0])).toBeCloseTo(2.4);
    expect(Math.abs(v![1][1] - v![0][1])).toBeCloseTo(2.4);
    const [big] = marksOf(D, [55, 35], 20);
    expect(Math.abs(big![1][0] - big![0][0])).toBeCloseTo(6);
  });

  it('sin solape o con un solo tramo no hay marcas', () => {
    expect(marksOf(D, [55, 35], 0)).toEqual([]);
    expect(marksOf(D, [200, 200], 5)).toEqual([]);
  });
});


/** Los mismos casos que comprueba el motor (`crates/tlaku-pipeline/tests/cross.rs`). */
describe('casos comunes con el motor', () => {
  interface Case {
    name: string;
    design: Bounds;
    size: [number, number];
    overlap: number;
    order: TileOrder;
    margin: number;
    expect: {
      cols: number;
      rows: number;
      colRanges: [number, number][];
      rowRanges: [number, number][];
      tiles?: { rect: TileSpec['rect']; offset: [number, number]; row: number; col: number }[];
      marks?: MarkSegment[];
      marksCount?: number;
    };
  }
  const file = JSON.parse(readFileSync(resolve(__dirname, '../../../../../crates/tlaku-pipeline/tests/cross/tiling.json'), 'utf8')) as {
    tolerance: number;
    cases: Case[];
    errors: { name: string; design: Bounds; size: [number, number]; overlap: number; expectError: string }[];
  };
  const tol = file.tolerance;
  const close = (got: readonly number[], want: readonly number[], what: string) => {
    expect(got.length, what).toBe(want.length);
    want.forEach((w, i) => expect(Math.abs(got[i]! - w), `${what}[${i}]: ${got[i]} vs ${w}`).toBeLessThanOrEqual(tol));
  };

  it.each(file.cases.map((c) => [c.name, c] as const))('%s', (_name, c) => {
    const r = mockPlanTiles({ design: c.design, size: c.size, overlap: c.overlap, order: c.order, margin: c.margin });
    if (!r.ok) throw new Error(`falló: ${r.error.kind}`);
    const p = r.plan;
    expect([p.cols, p.rows]).toEqual([c.expect.cols, c.expect.rows]);
    close(p.colRanges.flat(), c.expect.colRanges.flat(), 'colRanges');
    close(p.rowRanges.flat(), c.expect.rowRanges.flat(), 'rowRanges');
    if (c.expect.tiles) {
      expect(p.tiles).toHaveLength(c.expect.tiles.length);
      c.expect.tiles.forEach((w, i) => {
        const g = p.tiles[i]!;
        expect([g.row, g.col], `tramo ${i}`).toEqual([w.row, w.col]);
        close(g.spec.rect.flat(), w.rect.flat(), `rect ${i}`);
        close(g.spec.offset, w.offset, `offset ${i}`);
      });
    }
    if (c.expect.marks) close(p.marks.flat(2), c.expect.marks.flat(2), 'marks');
    if (c.expect.marksCount !== undefined) expect(p.marks).toHaveLength(c.expect.marksCount);
  });

  it.each(file.errors.map((c) => [c.name, c] as const))('error: %s', (_name, c) => {
    const r = mockPlanTiles({ design: c.design, size: c.size, overlap: c.overlap, order: 'rows', margin: 0 });
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.error.kind).toBe(c.expectError);
  });
});
