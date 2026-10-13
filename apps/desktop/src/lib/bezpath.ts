import type { BezPath } from '@/bindings/BezPath';
import type { PathSeg } from '@/bindings/PathSeg';
import type { Affine, Point } from '@/api/types';

export type { BezPath, PathSeg };

/**
 * Curvas Bézier del documento (mismo formato que `tlaku_core::bezpath`): segmentos en
 * coordenadas absolutas, varios subtrayectos por trayectoria (cada uno empieza con `move_to`).
 */

/** Constante clásica para aproximar un cuarto de círculo con una cúbica. */
export const KAPPA = 0.5522847498307936;

/** Error máximo (mm) con el que la interfaz aplana curvas para hit-test, cajas y el mock. */
export const FLATTEN_TOLERANCE_MM = 0.01;

/** Trayectoria de rectas por los puntos dados (cerrada con `close`). Vacía si no hay puntos. */
export function polylineBez(points: readonly Point[], closed: boolean): BezPath {
  if (points.length === 0) return [];
  const path: PathSeg[] = points.map((to, i) => (i === 0 ? { type: 'move_to', to } : { type: 'line_to', to }));
  if (closed) path.push({ type: 'close' });
  return path;
}

/** Elipse centrada en (cx, cy) como cuatro cúbicas, antihoraria desde el extremo derecho. */
export function ellipseBez(cx: number, cy: number, rx: number, ry: number): BezPath {
  const kx = rx * KAPPA;
  const ky = ry * KAPPA;
  return [
    { type: 'move_to', to: [cx + rx, cy] },
    { type: 'cubic_to', ctrl1: [cx + rx, cy + ky], ctrl2: [cx + kx, cy + ry], to: [cx, cy + ry] },
    { type: 'cubic_to', ctrl1: [cx - kx, cy + ry], ctrl2: [cx - rx, cy + ky], to: [cx - rx, cy] },
    { type: 'cubic_to', ctrl1: [cx - rx, cy - ky], ctrl2: [cx - kx, cy - ry], to: [cx, cy - ry] },
    { type: 'cubic_to', ctrl1: [cx + kx, cy - ry], ctrl2: [cx + rx, cy - ky], to: [cx + rx, cy] },
    { type: 'close' },
  ];
}

/**
 * Rectángulo antihorario desde la esquina inferior izquierda (esquinas en cualquier orden). Con
 * `radius` > 0 las esquinas son cúbicas reales; el radio se limita a la mitad del lado menor.
 */
export function roundedRectBez(x0: number, y0: number, x1: number, y1: number, radius = 0): BezPath {
  const minX = Math.min(x0, x1);
  const maxX = Math.max(x0, x1);
  const minY = Math.min(y0, y1);
  const maxY = Math.max(y0, y1);
  const r = Math.min(Math.max(0, radius), (maxX - minX) / 2, (maxY - minY) / 2);
  if (r <= 1e-9) {
    return polylineBez([[minX, minY], [maxX, minY], [maxX, maxY], [minX, maxY]], true);
  }
  const k = r * KAPPA;
  const line = (x: number, y: number): PathSeg => ({ type: 'line_to', to: [x, y] });
  return [
    { type: 'move_to', to: [minX + r, minY] },
    line(maxX - r, minY),
    { type: 'cubic_to', ctrl1: [maxX - r + k, minY], ctrl2: [maxX, minY + r - k], to: [maxX, minY + r] },
    line(maxX, maxY - r),
    { type: 'cubic_to', ctrl1: [maxX, maxY - r + k], ctrl2: [maxX - r + k, maxY], to: [maxX - r, maxY] },
    line(minX + r, maxY),
    { type: 'cubic_to', ctrl1: [minX + r - k, maxY], ctrl2: [minX, maxY - r + k], to: [minX, maxY - r] },
    line(minX, minY + r),
    { type: 'cubic_to', ctrl1: [minX, minY + r - k], ctrl2: [minX + r - k, minY], to: [minX + r, minY] },
    { type: 'close' },
  ];
}

/** Aplica una afín a todos los puntos (destinos y controles): una Bézier afín es otra Bézier. */
export function transformBez(path: BezPath, m: Affine): BezPath {
  const f = ([x, y]: Point): Point => [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]];
  return path.map((s): PathSeg => {
    switch (s.type) {
      case 'move_to':
      case 'line_to':
        return { type: s.type, to: f(s.to) };
      case 'quad_to':
        return { type: 'quad_to', ctrl: f(s.ctrl), to: f(s.to) };
      case 'cubic_to':
        return { type: 'cubic_to', ctrl1: f(s.ctrl1), ctrl2: f(s.ctrl2), to: f(s.to) };
      case 'close':
        return s;
    }
  });
}

/** Puntos que guarda una trayectoria (destinos y controles; `close` no cuenta): lo que pesa en memoria. */
export function bezPointCount(path: BezPath): number {
  let n = 0;
  for (const s of path) {
    if (s.type === 'quad_to') n += 2;
    else if (s.type === 'cubic_to') n += 3;
    else if (s.type !== 'close') n++;
  }
  return n;
}

/** Valores de t en (0, 1) donde una cúbica 1D (a, b, c, d) tiene un extremo. */
export function cubicExtrema(a: number, b: number, c: number, d: number): number[] {
  const qa = -a + 3 * b - 3 * c + d;
  const qb = 2 * (a - 2 * b + c);
  const qc = b - a;
  const inside = (t: number) => t > 0 && t < 1;
  if (Math.abs(qa) < 1e-12) return Math.abs(qb) > 1e-12 && inside(-qc / qb) ? [-qc / qb] : [];
  const disc = qb * qb - 4 * qa * qc;
  if (disc < 0) return [];
  const sq = Math.sqrt(disc);
  return [(-qb + sq) / (2 * qa), (-qb - sq) / (2 * qa)].filter(inside);
}

export function cubicAt(p0: Point, c1: Point, c2: Point, p1: Point, t: number): Point {
  const u = 1 - t;
  const [a, b, c, d] = [u * u * u, 3 * u * u * t, 3 * u * t * t, t * t * t];
  return [a * p0[0] + b * c1[0] + c * c2[0] + d * p1[0], a * p0[1] + b * c1[1] + c * c2[1] + d * p1[1]];
}

/** Puntos extremos (en X o Y) de una cúbica, para cajas exactas. */
export function cubicExtremePoints(p0: Point, c1: Point, c2: Point, p1: Point): Point[] {
  return [
    ...cubicExtrema(p0[0], c1[0], c2[0], p1[0]),
    ...cubicExtrema(p0[1], c1[1], c2[1], p1[1]),
  ].map((t) => cubicAt(p0, c1, c2, p1, t));
}

/** Una cuadrática elevada a cúbica (mismos extremos). */
export function quadToCubic(p0: Point, c: Point, p1: Point): [Point, Point] {
  const lerp = (a: Point, b: Point): Point => [a[0] + ((b[0] - a[0]) * 2) / 3, a[1] + ((b[1] - a[1]) * 2) / 3];
  return [lerp(p0, c), lerp(p1, c)];
}

/** Caja **exacta** [[minX, minY], [maxX, maxY]] de lo que dibuja (extremos de cada Bézier). */
export function bezBounds(list: readonly BezPath[]): [Point, Point] | null {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  const add = (p: Point) => {
    if (p[0] < minX) minX = p[0];
    if (p[0] > maxX) maxX = p[0];
    if (p[1] < minY) minY = p[1];
    if (p[1] > maxY) maxY = p[1];
  };
  for (const path of list) {
    let cur: Point | null = null;
    let start: Point | null = null;
    let pending = false;
    for (const s of path) {
      if (s.type === 'move_to') {
        cur = start = s.to;
        pending = true;
        continue;
      }
      if (s.type === 'close') {
        if (!pending && start) add(start);
        cur = start;
        continue;
      }
      const p0: Point = cur ?? s.to;
      if (pending) add(p0);
      pending = false;
      add(s.to);
      if (s.type === 'quad_to') {
        const [c1, c2] = quadToCubic(p0, s.ctrl, s.to);
        cubicExtremePoints(p0, c1, c2, s.to).forEach(add);
      } else if (s.type === 'cubic_to') {
        cubicExtremePoints(p0, s.ctrl1, s.ctrl2, s.to).forEach(add);
      }
      cur = s.to;
    }
  }
  return minX === Infinity ? null : [[minX, minY], [maxX, maxY]];
}

export interface FlatPath {
  points: Point[];
  closed: boolean;
}

function segmentsFor(maxSecondDiff: number, degreeFactor: number, tol: number): number {
  const n = Math.ceil(Math.sqrt((degreeFactor * maxSecondDiff) / tol));
  return Number.isFinite(n) ? Math.min(1000, Math.max(1, n)) : 1;
}

const secondDiff = (a: Point, b: Point, c: Point) => Math.hypot(a[0] - 2 * b[0] + c[0], a[1] - 2 * b[1] + c[1]);

/**
 * Aplana con error máximo `tol` (mm): mismo algoritmo que `BezPath::flatten` del motor (fórmula
 * de Wang). Un subtrayecto es cerrado si termina en `close` o si su último punto coincide con el primero.
 */
export function flattenBez(path: BezPath, tol = FLATTEN_TOLERANCE_MM): FlatPath[] {
  const out: FlatPath[] = [];
  let cur: Point[] = [];
  const finish = (closed: boolean) => {
    const pts: Point[] = [];
    for (const p of cur) {
      const last = pts[pts.length - 1];
      if (!last || Math.hypot(p[0] - last[0], p[1] - last[1]) >= 1e-9) pts.push(p);
    }
    cur = [];
    let isClosed = closed;
    if (pts.length > 2 && Math.hypot(pts[0]![0] - pts[pts.length - 1]![0], pts[0]![1] - pts[pts.length - 1]![1]) < 1e-9) {
      pts.pop();
      isClosed = true;
    }
    if (pts.length >= 2) out.push({ points: pts, closed: isClosed });
  };
  for (const s of path) {
    switch (s.type) {
      case 'move_to':
        finish(false);
        cur.push(s.to);
        break;
      case 'line_to':
        cur.push(s.to);
        break;
      case 'quad_to': {
        const p0 = cur[cur.length - 1];
        if (!p0) {
          cur.push(s.to);
          break;
        }
        const n = segmentsFor(secondDiff(p0, s.ctrl, s.to), 0.25, tol);
        for (let i = 1; i <= n; i++) {
          const t = i / n;
          const u = 1 - t;
          cur.push([
            u * u * p0[0] + 2 * u * t * s.ctrl[0] + t * t * s.to[0],
            u * u * p0[1] + 2 * u * t * s.ctrl[1] + t * t * s.to[1],
          ]);
        }
        break;
      }
      case 'cubic_to': {
        const p0 = cur[cur.length - 1];
        if (!p0) {
          cur.push(s.to);
          break;
        }
        const d = Math.max(secondDiff(p0, s.ctrl1, s.ctrl2), secondDiff(s.ctrl1, s.ctrl2, s.to));
        const n = segmentsFor(d, 0.75, tol);
        for (let i = 1; i <= n; i++) cur.push(cubicAt(p0, s.ctrl1, s.ctrl2, s.to, i / n));
        break;
      }
      case 'close': {
        const start = cur[0];
        finish(true);
        if (start) cur.push(start);
        break;
      }
    }
  }
  finish(false);
  return out;
}
