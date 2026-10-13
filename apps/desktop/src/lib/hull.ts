import type { Point } from '@/api/types';

/**
 * Casco convexo (cadena monótona de Andrew) en sentido antihorario, sin puntos repetidos ni
 * alineados, empezando por el más bajo (a igual Y, el de más a la izquierda). Ignora lo que no sea
 * un número. Con todos los puntos iguales devuelve uno; con todos alineados, los dos extremos.
 * Es la misma regla que `tlaku_grbl::frame::convex_hull`, que es la que usa la máquina.
 */
export function convexHull(points: readonly Point[]): Point[] {
  const pts = points
    .filter((p) => Number.isFinite(p[0]) && Number.isFinite(p[1]))
    .sort((a, b) => a[0] - b[0] || a[1] - b[1])
    .filter((p, i, all) => i === 0 || p[0] !== all[i - 1]![0] || p[1] !== all[i - 1]![1]);
  if (pts.length < 3) return pts;
  const cross = (a: Point, b: Point, c: Point) => (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
  const hull: Point[] = [];
  const push = (p: Point, minLen: number) => {
    while (hull.length >= minLen && cross(hull[hull.length - 2]!, hull[hull.length - 1]!, p) <= 0) hull.pop();
    hull.push(p);
  };
  for (const p of pts) push(p, 2);
  const lower = hull.length + 1;
  for (let i = pts.length - 2; i >= 0; i--) push(pts[i]!, lower);
  hull.pop(); // el último repite al primero
  let start = 0;
  hull.forEach((p, i) => {
    const s = hull[start]!;
    if (p[1] < s[1] || (p[1] === s[1] && p[0] < s[0])) start = i;
  });
  return [...hull.slice(start), ...hull.slice(0, start)];
}
