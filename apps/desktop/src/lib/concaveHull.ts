import type { Point } from '@/api/types';
import { convexHull } from './hull';

const cross = (a: Point, b: Point, c: Point) => (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
const dist = (a: Point, b: Point) => Math.hypot(a[0] - b[0], a[1] - b[1]);

function properlyCross(a: Point, b: Point, c: Point, d: Point): boolean {
  const d1 = cross(a, b, c);
  const d2 = cross(a, b, d);
  const d3 = cross(c, d, a);
  const d4 = cross(c, d, b);
  return ((d1 > 0 && d2 < 0) || (d1 < 0 && d2 > 0)) && ((d3 > 0 && d4 < 0) || (d3 < 0 && d4 > 0));
}

/** Ángulo en `p` bajo el que se ve la arista `ab`. */
function angleAt(p: Point, a: Point, b: Point): number {
  const u: Point = [a[0] - p[0], a[1] - p[1]];
  const v: Point = [b[0] - p[0], b[1] - p[1]];
  return Math.atan2(Math.abs(u[0] * v[1] - u[1] * v[0]), u[0] * v[0] + u[1] * v[1]);
}

const same = (a: Point, b: Point) => a[0] === b[0] && a[1] === b[1];

/**
 * Casco cóncavo: parte del convexo y cambia cada arista más larga que `maxEdge` por dos aristas
 * hacia el punto interior que la ve bajo el mayor ángulo, mientras no cruce el contorno. Es la
 * misma regla que `tlaku_geom::concave_hull` (el motor real); con `maxEdge` no positivo o muy
 * grande devuelve el casco convexo.
 */
export function concaveHull(points: readonly Point[], maxEdge: number): Point[] {
  const poly = convexHull(points);
  if (poly.length < 3 || !(Number.isFinite(maxEdge) && maxEdge > 0)) return poly;
  const seen = new Set<string>();
  let interior = points.filter((p) => {
    const k = `${p[0]},${p[1]}`;
    if (!Number.isFinite(p[0]) || !Number.isFinite(p[1]) || seen.has(k) || poly.some((q) => same(p, q))) return false;
    seen.add(k);
    return true;
  });
  const MAX_INTERIOR = 4000;
  if (interior.length > MAX_INTERIOR) {
    const step = Math.ceil(interior.length / MAX_INTERIOR);
    interior = interior.filter((_, i) => i % step === 0);
  }
  const stuck = new Set<string>();
  const key = (a: Point, b: Point) => `${a[0]},${a[1]}|${b[0]},${b[1]}`;
  for (;;) {
    const n = poly.length;
    let pick = -1;
    let longest = maxEdge;
    for (let i = 0; i < n; i++) {
      const a = poly[i]!;
      const b = poly[(i + 1) % n]!;
      const len = dist(a, b);
      if (len > longest && !stuck.has(key(a, b))) {
        longest = len;
        pick = i;
      }
    }
    if (pick < 0) break;
    const a = poly[pick]!;
    const b = poly[(pick + 1) % n]!;
    const len = dist(a, b);
    const cands = interior
      .map((p, k) => ({ k, p }))
      .filter(({ p }) => cross(a, b, p) > 0 && dist(p, a) < len && dist(p, b) < len)
      .map(({ k, p }) => ({ k, angle: angleAt(p, a, b) }))
      .sort((x, y) => y.angle - x.angle);
    const chosen = cands.find(({ k }) => {
      const p = interior[k]!;
      for (let j = 0; j < n; j++) {
        if (j === pick) continue;
        const c = poly[j]!;
        const d = poly[(j + 1) % n]!;
        if (properlyCross(a, p, c, d) || properlyCross(p, b, c, d)) return false;
      }
      return true;
    });
    if (chosen) {
      poly.splice(pick + 1, 0, interior[chosen.k]!);
      interior.splice(chosen.k, 1);
    } else {
      stuck.add(key(a, b));
    }
  }
  return poly;
}
