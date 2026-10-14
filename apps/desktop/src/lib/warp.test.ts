import { describe, expect, it } from 'vitest';
import type { Point, WarpGrid } from '@/api/types';
import { concaveHull } from './concaveHull';
import { convexHull } from './hull';
import { homography, identityEnvelope, identityGrid, lagrange4, makeWarp } from './warp';
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

describe('malla de 4x4 puntos', () => {
  const sq: Bounds = [[0, 0], [30, 30]];

  it('los pesos de Lagrange valen 1 en su nodo, 0 en los demás y suman 1', () => {
    [0, 1 / 3, 2 / 3, 1].forEach((t, k) => lagrange4(t).forEach((w, i) => expect(w).toBeCloseTo(i === k ? 1 : 0, 12)));
    expect(lagrange4(0.37).reduce((a, b) => a + b, 0)).toBeCloseTo(1, 12);
  });

  it('la malla uniforme es la identidad', () => {
    const warp = makeWarp(sq, { ...identityEnvelope(sq), grid: identityGrid(sq) })!;
    for (const p of [[0, 0], [30, 30], [7, 21], [13.3, 4.4]] as Point[]) near(warp(p), p);
  });

  it('la superficie pasa por cada punto de la malla', () => {
    const grid = identityGrid(sq);
    grid[1]![1] = [8, 14];
    grid[2]![1] = [12, 22];
    grid[0]![3] = [33, -4];
    const warp = makeWarp(sq, { ...identityEnvelope(sq), grid })!;
    grid.forEach((row, j) => row.forEach((target, i) => near(warp([10 * i, 10 * j]), target)));
  });

  it('es determinista', () => {
    const grid: WarpGrid = identityGrid(sq);
    grid[2]![2] = [17, 25];
    const a = makeWarp(sq, { ...identityEnvelope(sq), grid })!;
    const b = makeWarp(sq, { ...identityEnvelope(sq), grid })!;
    expect(a([11.1, 22.2])).toEqual(b([11.1, 22.2]));
  });
});

describe('perspectiva de cuatro esquinas', () => {
  const rect: Bounds = [[0, 0], [20, 10]];
  const quad: [Point, Point, Point, Point] = [[0, 0], [20, 0], [15, 10], [5, 10]];

  it('lleva cada esquina a la suya y conserva las rectas', () => {
    const warp = makeWarp(rect, { ...identityEnvelope(rect), perspective: quad })!;
    near(warp([0, 0]), quad[0]);
    near(warp([20, 0]), quad[1]);
    near(warp([20, 10]), quad[2]);
    near(warp([0, 10]), quad[3]);
    const [a, m, c] = [warp([2, 1]), warp([9, 4]), warp([16, 7])];
    expect((m[0] - a[0]) * (c[1] - a[1]) - (m[1] - a[1]) * (c[0] - a[0])).toBeCloseTo(0, 9);
  });

  it('un cuadrilátero cóncavo o degenerado no define proyección', () => {
    expect(homography([[0, 0], [10, 0], [2, 2], [0, 10]])).toBeNull();
    expect(homography([[0, 0], [5, 0], [10, 0], [15, 0]])).toBeNull();
    expect(makeWarp(rect, { ...identityEnvelope(rect), perspective: [[0, 0], [10, 0], [2, 2], [0, 10]] })).toBeNull();
  });
});
