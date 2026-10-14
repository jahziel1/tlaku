import { describe, expect, it } from 'vitest';
import type { DesignObject, Point } from '@/api/types';
import { polylineBez } from '@/lib/bezpath';
import { cumulativeLength, nearestOnContour, pointAtArc, polyPoints, spanAround, startMark } from './contour';
import { getGeom } from './geom';

const SQUARE: Point[] = [[0, 0], [10, 0], [10, 10], [0, 10]];

function obj(points: Point[], closed: boolean, extra: Partial<DesignObject> = {}): DesignObject {
  return {
    id: 'o', layer: '#ff0000', transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths: [polylineBez(points, closed)] },
    locked: false, hidden: false, group: null, ...extra,
  };
}

describe('contorno más cercano', () => {
  it('lleva el clic al borde y mide la longitud de arco', () => {
    const hit = nearestOnContour(obj(SQUARE, true), [4, -3])!;
    expect(hit.point).toEqual([4, 0]);
    expect(hit.dist).toBeCloseTo(3);
    expect(hit.arc).toBeCloseTo(4);
    expect(hit.closed).toBe(true);
  });

  it('incluye el lado de cierre de un contorno cerrado', () => {
    const hit = nearestOnContour(obj(SQUARE, true), [-2, 5])!;
    expect(hit.point).toEqual([0, 5]);
    expect(hit.arc).toBeCloseTo(35);
  });

  it('puede limitarse a los cerrados y no falla con objetos sin trazo', () => {
    const open = obj([[0, 0], [10, 0]], false);
    expect(nearestOnContour(open, [5, 1], true)).toBeNull();
    expect(nearestOnContour(open, [5, 1])!.closed).toBe(false);
    expect(nearestOnContour(obj([], false), [0, 0])).toBeNull();
  });
});

describe('tramos y sentido', () => {
  const pts = polyPoints(getGeom(obj(SQUARE, true)).polys[0]!);
  const cum = cumulativeLength(pts, true);

  it('perímetro y punto a una longitud de arco', () => {
    expect(cum[cum.length - 1]).toBeCloseTo(40);
    expect(pointAtArc(pts, cum, true, 15).point).toEqual([10, 5]);
    expect(pointAtArc(pts, cum, true, 45).point).toEqual([5, 0]);
  });

  it('el tramo alrededor de un punto sigue las esquinas y da la vuelta al inicio', () => {
    const around = spanAround(pts, true, 10, 2);
    expect(around[0]).toEqual([8, 0]);
    expect(around).toContainEqual([10, 0]);
    expect(around[around.length - 1]).toEqual([10, 2]);
    const wrap = spanAround(pts, true, 1, 2);
    expect(wrap[0]).toEqual([0, 1]);
    expect(wrap[wrap.length - 1]).toEqual([3, 0]);
  });
});

describe('marca de inicio', () => {
  it('sin inicio fijado es el del trazo y no está fijado', () => {
    const m = startMark(obj(SQUARE, true))!;
    expect(m.point).toEqual([0, 0]);
    expect(m.dir).toEqual([1, 0]);
    expect(m.fixed).toBe(false);
  });

  it('el punto elegido manda y el sentido invertido da la vuelta a la flecha', () => {
    const m = startMark(obj(SQUARE, true, { start: { point: [10, 10], reverse: false } }))!;
    expect(m.point).toEqual([10, 10]);
    expect(m.dir).toEqual([-1, 0]);
    expect(m.fixed).toBe(true);
    const r = startMark(obj(SQUARE, true, { start: { point: [10, 10], reverse: true } }))!;
    expect(r.dir[0]).toBeCloseTo(0);
    expect(r.dir[1]).toBeCloseTo(-1);
  });

  it('en un trazo abierto invertir cambia el extremo de salida', () => {
    const m = startMark(obj([[0, 0], [10, 0]], false, { start: { point: null, reverse: true } }))!;
    expect(m.point).toEqual([10, 0]);
    expect(m.dir[0]).toBeCloseTo(-1);
  });

  it('sin trazo no hay marca', () => {
    expect(startMark(obj([], false))).toBeNull();
  });
});

describe('flecha de inicio del trazo tal como se dibujó', () => {
  /** Lo que daba la implementación general (arco 0 sobre los puntos y las longitudes). */
  function viaArc(o: DesignObject) {
    const poly = getGeom(o).polys.findIndex((p) => p.pts.length >= 2);
    const pts = polyPoints(getGeom(o).polys[poly]!);
    const closed = getGeom(o).polys[poly]!.closed;
    const at = pointAtArc(pts, cumulativeLength(pts, closed), closed, 0);
    return { point: at.point, dir: at.dir, poly };
  }

  it('da lo mismo que el cálculo por longitud de arco, también con puntos repetidos al inicio y trayectos sin largo', () => {
    const casos: DesignObject[] = [
      obj(SQUARE, true),
      obj(SQUARE, false),
      obj([[0, 0], [0, 0], [0, 0], [3, 4], [9, 9]], false), // tramos de largo cero al principio
      obj([[5, 5], [5, 5]], true), // sin largo: dirección por omisión
      obj([[0, 0], [10, 0], [10, 10]], true, { transform: [0, 1, -1, 0, 5, 5] }), // girado
    ];
    for (const o of casos) {
      const m = startMark(o)!;
      const want = viaArc(o);
      expect(m.point[0]).toBeCloseTo(want.point[0], 12);
      expect(m.point[1]).toBeCloseTo(want.point[1], 12);
      expect(m.dir[0]).toBeCloseTo(want.dir[0], 12);
      expect(m.dir[1]).toBeCloseTo(want.dir[1], 12);
      expect(m.poly).toBe(want.poly);
      expect(m.fixed).toBe(false);
    }
  });

  it('con un trayecto de 200 000 puntos no arma arreglos de puntos (es barato)', () => {
    const pts: Point[] = Array.from({ length: 200_000 }, (_, i) => [i * 0.001, (i % 7) * 0.01]);
    const o = obj(pts, false);
    getGeom(o);
    const t0 = performance.now();
    const m = startMark(o)!;
    expect(performance.now() - t0).toBeLessThan(20);
    expect(m.point).toEqual([0, 0]);
  });
});
