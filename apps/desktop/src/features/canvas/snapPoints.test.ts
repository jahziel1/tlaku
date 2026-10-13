import { describe, expect, it } from 'vitest';
import { ellipseBez } from '@/lib/bezpath';
import { getGeom } from './geom';
import { collectSnapTargets, snapPoint, snapToPoints, type SnapOptions } from './snap';
import { intersectionsNear, nearestSnapPoint, objectSnapPoints, segmentIntersection } from './snapPoints';
import { newVectorObject } from './shapes';
import { lineObj, rectObj } from './testUtil';

const bed = { width: 400, height: 300 };
const opts = (over: Partial<SnapOptions> = {}): SnapOptions => ({ tol: 2, grid: null, objects: true, ...over });

describe('puntos de ajuste de un objeto', () => {
  it('un rectángulo da cuatro nodos, cuatro puntos medios y un centro', () => {
    const pts = objectSnapPoints(rectObj('r', 0, 0, 10, 6));
    expect(pts.filter((p) => p.kind === 'node')).toHaveLength(4);
    expect(pts.filter((p) => p.kind === 'mid').map((p) => p.p).sort()).toEqual([[0, 3], [10, 3], [5, 0], [5, 6]].sort());
    expect(pts.filter((p) => p.kind === 'center')[0]!.p).toEqual([5, 3]);
  });

  it('una curva aporta su nodo de llegada y el punto de la curva a mitad de camino', () => {
    const pts = objectSnapPoints(newVectorObject('e', '#ff0000', [ellipseBez(0, 0, 10, 10)]));
    expect(pts.filter((p) => p.kind === 'node')).toHaveLength(4);
    const mid = pts.filter((p) => p.kind === 'mid');
    expect(mid).toHaveLength(4);
    for (const m of mid) expect(Math.hypot(m.p[0], m.p[1])).toBeCloseTo(10, 1); // sobre el círculo
    expect(pts.filter((p) => p.kind === 'center')[0]!.p).toEqual([0, 0]);
  });
});

describe('ajuste a puntos', () => {
  const objs = [rectObj('a', 100, 100, 200, 160)];
  const set = collectSnapTargets(objs, new Set(), bed);

  it('el cursor cerca de un nodo se pega exacto', () => {
    const r = snapPoint([200.8, 99.4], set, opts());
    expect(r.point).toMatchObject({ kind: 'node', p: [200, 100] });
    expect(r.dx).toBeCloseTo(-0.8, 9);
    expect(r.dy).toBeCloseTo(0.6, 9);
  });

  it('puntos medios y centros', () => {
    expect(snapToPoints([150.5, 100.5], set, opts())?.kind).toBe('mid');
    expect(snapToPoints([150.5, 130.5], set, opts())?.kind).toBe('center');
  });

  it('fuera de la tolerancia no hay punto (queda el ajuste por ejes)', () => {
    expect(snapToPoints([150, 140], set, opts())).toBeNull();
  });

  it('no ajusta si el ajuste a objetos está apagado', () => {
    expect(snapToPoints([200, 100], set, opts({ objects: false }))).toBeNull();
  });

  it('un objeto omitido (el que se arrastra) no ofrece sus puntos', () => {
    const skipped = collectSnapTargets(objs, new Set(['a']), bed);
    expect(snapToPoints([200, 100], skipped, opts())).toBeNull();
  });

  it('a igual distancia gana el tipo más específico', () => {
    const best = nearestSnapPoint(
      [{ p: [1, 1], kind: 'center' }, { p: [1, 1], kind: 'node' }, { p: [1, 1], kind: 'inter' }],
      [1, 1],
      1,
    );
    expect(best?.kind).toBe('inter');
  });
});

describe('intersecciones', () => {
  it('cruce de dos segmentos, sin contar los extremos compartidos', () => {
    expect(segmentIntersection([0, 0, 10, 10], [0, 10, 10, 0])).toEqual([5, 5]);
    expect(segmentIntersection([0, 0, 10, 0], [10, 0, 10, 10])).toBeNull(); // se tocan en un extremo
    expect(segmentIntersection([0, 0, 10, 0], [0, 5, 10, 5])).toBeNull(); // paralelos
    expect(segmentIntersection([0, 0, 1, 1], [5, 0, 5, 10])).toBeNull(); // no se alcanzan
  });

  it('se hallan cerca del cursor y el ajuste las prefiere', () => {
    const x1 = lineObj('x1', [0, 0], [100, 100]);
    const x2 = lineObj('x2', [0, 100], [100, 0]);
    const geoms = [getGeom(x1), getGeom(x2)];
    expect(intersectionsNear(geoms, [50.5, 49.5], 2)).toEqual([{ p: [50, 50], kind: 'inter' }]);
    expect(intersectionsNear(geoms, [10, 10], 2)).toEqual([]);
    const set = collectSnapTargets([x1, x2], new Set(), bed);
    expect(snapToPoints([50.4, 50.2], set, opts())).toMatchObject({ kind: 'inter', p: [50, 50] });
  });
});
