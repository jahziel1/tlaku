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
