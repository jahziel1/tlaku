import { describe, expect, it } from 'vitest';
import type { Point } from '@/api/types';
import { concaveHull } from './concaveHull';
import { convexHull } from './hull';
import { identityEnvelope, makeWarp } from './warp';
import type { Bounds } from './geometry';

const box: Bounds = [[0, 0], [20, 10]];
const near = (a: Point, b: Point, digits = 9) => a.forEach((v, i) => expect(v).toBeCloseTo(b[i]!, digits));

describe('deformación por envolvente', () => {
  it('la envolvente identidad deja cada punto donde está', () => {
    const warp = makeWarp(box, identityEnvelope(box))!;
    for (const p of [[0, 0], [20, 10], [5, 3], [13.5, 8]] as Point[]) near(warp(p), p);
  });

  it('mover una esquina mueve esa esquina y los bordes se estiran con ella', () => {
    const env = identityEnvelope(box);
    env.top = [[0, 10], [20 / 3, 10], [40 / 3, 15], [20, 15]];
    env.right = [[20, 0], [20, 5], [20, 10], [20, 15]];
    const warp = makeWarp(box, env)!;
    near(warp([20, 10]), [20, 15]);
    near(warp([0, 0]), [0, 0]);
    near(warp([20, 0]), [20, 0]);
    near(warp([0, 10]), [0, 10]);
  });

  it('un borde curvo alza el centro de la forma', () => {
    const env = identityEnvelope(box);
    for (const e of [env.bottom, env.top]) {
      e[1] = [e[1][0], e[1][1] + 16 / 3];
      e[2] = [e[2][0], e[2][1] + 16 / 3];
    }
    const warp = makeWarp(box, env)!;
    near(warp([10, 0]), [10, 4]);
    near(warp([10, 10]), [10, 14]);
    near(warp([0, 5]), [0, 5]);
  });

  it('una región sin ancho o sin alto no se puede deformar', () => {
    expect(makeWarp([[0, 0], [0, 10]], identityEnvelope([[0, 0], [1, 10]]))).toBeNull();
  });
});

describe('casco cóncavo (motor simulado)', () => {
  const c: Point[] = [];
  for (const r of [8, 10, 12]) for (let k = 0; k < 60; k++) {
    const a = ((40 + (k * 280) / 59) * Math.PI) / 180;
    c.push([r * Math.cos(a), r * Math.sin(a)]);
  }
  const area = (poly: Point[]) => poly.reduce((s, p, i) => s + (p[0] * poly[(i + 1) % poly.length]![1] - poly[(i + 1) % poly.length]![0] * p[1]), 0) / 2;

  it('es menor que el convexo y se queda en los puntos dados', () => {
    const convex = convexHull(c);
    const concave = concaveHull(c, 6);
    expect(area(concave)).toBeGreaterThan(0);
    expect(area(concave)).toBeLessThan(area(convex) - 20);
    for (const p of concave) expect(c.some((q) => q[0] === p[0] && q[1] === p[1])).toBe(true);
  });

  it('con una arista máxima enorme o inválida es el convexo', () => {
    expect(concaveHull(c, 1e6)).toEqual(convexHull(c));
    expect(concaveHull(c, 0)).toEqual(convexHull(c));
    expect(concaveHull(c, Number.NaN)).toEqual(convexHull(c));
  });
});
