import { describe, expect, it } from 'vitest';
import { bezBounds, bezPointCount, ellipseBez, flattenBez, KAPPA, polylineBez, transformBez, type BezPath } from './bezpath';
import { objectBounds, objectPolylines } from './geometry';
import type { DesignObject } from '@/api/types';

const obj = (paths: BezPath[], transform: DesignObject['transform'] = [1, 0, 0, 1, 0, 0]): DesignObject => ({
  id: 'a', layer: '#ff0000', transform, kind: { type: 'vector', paths }, locked: false, hidden: false, group: null,
});

describe('curvas Bézier', () => {
  it('una polilínea sobrevive al aplanado tal cual', () => {
    const pts: [number, number][] = [[0, 0], [10, 0], [10, 5]];
    expect(flattenBez(polylineBez(pts, true))).toEqual([{ points: pts, closed: true }]);
    expect(flattenBez(polylineBez(pts, false))).toEqual([{ points: pts, closed: false }]);
  });

  it('la caja es exacta: usa los extremos de la curva y no los puntos de control', () => {
    const quad: BezPath = [{ type: 'move_to', to: [0, 0] }, { type: 'quad_to', ctrl: [5, 10], to: [10, 0] }];
    const b = bezBounds([quad])!;
    expect(b[1][1]).toBeCloseTo(5, 12); // el control está en y=10, la curva llega a 5
    const cubic: BezPath = [{ type: 'move_to', to: [0, 0] }, { type: 'cubic_to', ctrl1: [0, 10], ctrl2: [10, 10], to: [10, 0] }];
    expect(bezBounds([cubic])![1][1]).toBeCloseTo(7.5, 12);
    expect(bezBounds([])).toBeNull();
    expect(bezBounds([[{ type: 'move_to', to: [1, 1] }]])).toBeNull(); // un move_to suelto no dibuja
  });

  it('la caja de un círculo es el cuadrado de su diámetro, también girado o escalado', () => {
    const c = ellipseBez(0, 0, 10, 10);
    expect(bezBounds([c])).toEqual([[-10, -10], [10, 10]]);
    const o = obj([c], [2, 0, 0, 0.5, 100, 50]);
    const b = objectBounds(o)!;
    expect(b[0][0]).toBeCloseTo(80, 9);
    expect(b[1][0]).toBeCloseTo(120, 9);
    expect(b[0][1]).toBeCloseTo(45, 9);
    expect(b[1][1]).toBeCloseTo(55, 9);
  });

  it('transformar curvas lleva destinos y controles', () => {
    const t = transformBez(ellipseBez(0, 0, 1, 1), [1, 0, 0, 1, 5, 7]);
    expect(t[0]).toEqual({ type: 'move_to', to: [6, 7] });
    expect(t[1]).toEqual({ type: 'cubic_to', ctrl1: [6, 7 + KAPPA], ctrl2: [5 + KAPPA, 8], to: [5, 8] });
  });

  it('aplanar respeta la tolerancia en mm reales del documento (después del transform)', () => {
    const o = obj([ellipseBez(0, 0, 10, 10)], [10, 0, 0, 10, 0, 0]); // círculo de radio 100
    for (const poly of objectPolylines(o, 0.01)) {
      expect(poly.closed).toBe(true);
      // 0,01 de aplanado + ~0,03 % del radio que ya se desvía la cúbica de un círculo perfecto.
      for (const [x, y] of poly.points) expect(Math.abs(Math.hypot(x, y) - 100)).toBeLessThan(0.01 + 0.0003 * 100);
    }
    // Más fino da más puntos.
    expect(objectPolylines(o, 0.001)[0].points.length).toBeGreaterThan(objectPolylines(o, 0.1)[0].points.length);
  });

  it('cuenta los puntos que guarda (para el presupuesto del historial)', () => {
    expect(bezPointCount(polylineBez([[0, 0], [1, 1], [2, 0]], true))).toBe(3);
    expect(bezPointCount(ellipseBez(0, 0, 1, 1))).toBe(1 + 4 * 3);
  });

  it('un subtrayecto con extremo = inicio sin close se considera cerrado', () => {
    const p: BezPath = polylineBez([[0, 0], [4, 0], [4, 4], [0, 0]], false);
    expect(flattenBez(p)).toEqual([{ points: [[0, 0], [4, 0], [4, 4]], closed: true }]);
  });
});
