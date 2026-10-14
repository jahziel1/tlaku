import type { Point, WarpEnvelope, WarpGrid } from '@/api/types';
import type { Bounds } from './geometry';

/**
 * Deformación por envolvente (parche de Coons), la misma regla que `tlaku_geom::Envelope`: el
 * punto `(u, v)` del rectángulo de origen (0..1 en cada eje) va a la mezcla de los cuatro bordes
 * Bézier menos la corrección de las esquinas. Sirve al motor simulado y a las pruebas.
 */

type Edge = WarpEnvelope['bottom'];

function cubic(e: Edge, t: number): Point {
  const u = 1 - t;
  const a = u * u * u;
  const b = 3 * u * u * t;
  const c = 3 * u * t * t;
  const d = t * t * t;
  return [a * e[0][0] + b * e[1][0] + c * e[2][0] + d * e[3][0], a * e[0][1] + b * e[1][1] + c * e[2][1] + d * e[3][1]];
}

/** Ajusta los extremos de `left` y `right` a las esquinas de `bottom` y `top` (moviendo sus controles con ellos). */
export function snapEnvelope(env: WarpEnvelope): WarpEnvelope {
  const fix = (e: Edge, start: Point, end: Point): Edge => {
    const ds: Point = [start[0] - e[0][0], start[1] - e[0][1]];
    const de: Point = [end[0] - e[3][0], end[1] - e[3][1]];
    return [start, [e[1][0] + ds[0], e[1][1] + ds[1]], [e[2][0] + de[0], e[2][1] + de[1]], end];
  };
  return {
    bottom: env.bottom,
    top: env.top,
    left: fix(env.left, env.bottom[0], env.top[0]),
    right: fix(env.right, env.bottom[3], env.top[3]),
  };
}

/** Imagen de `(u, v)` bajo la envolvente (ya ajustada con `snapEnvelope`). */
export function envelopeAt(env: WarpEnvelope, u: number, v: number): Point {
  const b = cubic(env.bottom, u);
  const t = cubic(env.top, u);
  const l = cubic(env.left, v);
  const r = cubic(env.right, v);
  const c00 = env.bottom[0];
  const c10 = env.bottom[3];
  const c01 = env.top[0];
  const c11 = env.top[3];
  const w00 = (1 - u) * (1 - v);
  const w10 = u * (1 - v);
  const w01 = (1 - u) * v;
  const w11 = u * v;
  return [
    (1 - v) * b[0] + v * t[0] + (1 - u) * l[0] + u * r[0] - (c00[0] * w00 + c10[0] * w10 + c01[0] * w01 + c11[0] * w11),
    (1 - v) * b[1] + v * t[1] + (1 - u) * l[1] + u * r[1] - (c00[1] * w00 + c10[1] * w10 + c01[1] * w01 + c11[1] * w11),
  ];
}

/** Pesos de la interpolación cúbica de Lagrange en los nodos 0, 1/3, 2/3 y 1 (igual que `tlaku_geom`). */
export function lagrange4(t: number): [number, number, number, number] {
  const a = t - 1 / 3;
  const b = t - 2 / 3;
  const c = t - 1;
  return [-4.5 * a * b * c, 13.5 * t * b * c, -13.5 * t * a * c, 4.5 * t * a * b];
}

/** Imagen de `(u, v)` bajo la malla de 4x4 puntos: la superficie pasa por cada uno. */
export function meshAt(grid: WarpGrid, u: number, v: number): Point {
  const lu = lagrange4(u);
  const lv = lagrange4(v);
  let x = 0;
  let y = 0;
  for (let j = 0; j < 4; j++) {
    for (let i = 0; i < 4; i++) {
      const p = grid[j]![i]!;
      const w = lu[i]! * lv[j]!;
      x += p[0] * w;
      y += p[1] * w;
    }
  }
  return [x, y];
}

type Corners = NonNullable<WarpEnvelope['perspective']>;

/**
 * Coeficientes `[a, b, c, d, e, f, g, h]` de la proyección del cuadrado unidad a las cuatro esquinas
 * (abajo-izquierda, abajo-derecha, arriba-derecha, arriba-izquierda), o `null` si el cuadrilátero es
 * degenerado o cóncavo. Es la misma cuenta que `tlaku_geom::Warp::perspective`.
 */
export function homography(q: Corners): number[] | null {
  const [p0, p1, p2, p3] = q;
  const dx1 = p1[0] - p2[0];
  const dx2 = p3[0] - p2[0];
  const dy1 = p1[1] - p2[1];
  const dy2 = p3[1] - p2[1];
  const sx = p0[0] - p1[0] + p2[0] - p3[0];
  const sy = p0[1] - p1[1] + p2[1] - p3[1];
  if (![...p0, ...p1, ...p2, ...p3].every(Number.isFinite)) return null;
  const scale = Math.max(1, Math.abs(dx1), Math.abs(dx2), Math.abs(dy1), Math.abs(dy2));
  let g = 0;
  let h = 0;
  if (Math.abs(sx) > 1e-12 * scale || Math.abs(sy) > 1e-12 * scale) {
    const den = dx1 * dy2 - dx2 * dy1;
    if (Math.abs(den) <= 1e-12 * scale * scale) return null;
    g = (sx * dy2 - dx2 * sy) / den;
    h = (dx1 * sy - sx * dy1) / den;
  }
  if ([1, 1 + g, 1 + h, 1 + g + h].some((d) => !Number.isFinite(d) || d <= 1e-6)) return null;
  return [p1[0] - p0[0] + g * p1[0], p3[0] - p0[0] + h * p3[0], p0[0], p1[1] - p0[1] + g * p1[1], p3[1] - p0[1] + h * p3[1], p0[1], g, h];
}

/** Imagen de `(u, v)` bajo la proyección `m` (ver `homography`). */
export function projectiveAt(m: readonly number[], u: number, v: number): Point {
  const den = m[6]! * u + m[7]! * v + 1;
  return [(m[0]! * u + m[1]! * v + m[2]!) / den, (m[3]! * u + m[4]! * v + m[5]!) / den];
}

/**
 * Función de deformación de la región `bounds` a la envolvente, o `null` si la región no tiene ancho y
 * alto (o si la proyección de las esquinas no está definida). Manda la malla; luego la perspectiva;
 * si no, el parche de Coons de los cuatro bordes.
 */
export function makeWarp(bounds: Bounds, envelope: WarpEnvelope): ((p: Point) => Point) | null {
  const w = bounds[1][0] - bounds[0][0];
  const h = bounds[1][1] - bounds[0][1];
  if (!(w > 1e-9) || !(h > 1e-9)) return null;
  const uv = ([x, y]: Point): Point => [(x - bounds[0][0]) / w, (y - bounds[0][1]) / h];
  if (envelope.grid) {
    const grid = envelope.grid;
    return (p) => meshAt(grid, ...uv(p));
  }
  if (envelope.perspective) {
    const m = homography(envelope.perspective);
    return m ? (p) => projectiveAt(m, ...uv(p)) : null;
  }
  const env = snapEnvelope(envelope);
  return (p) => envelopeAt(env, ...uv(p));
}

/** Malla uniforme sobre la región (la identidad), de abajo hacia arriba y de izquierda a derecha. */
export function identityGrid(bounds: Bounds): WarpGrid {
  const [[x0, y0], [x1, y1]] = bounds;
  const row = (j: number): [Point, Point, Point, Point] => {
    const y = y0 + ((y1 - y0) * j) / 3;
    return [0, 1, 2, 3].map((i) => [x0 + ((x1 - x0) * i) / 3, y] as Point) as [Point, Point, Point, Point];
  };
  return [row(0), row(1), row(2), row(3)];
}

/** Envolvente que deja la región tal cual. */
export function identityEnvelope(bounds: Bounds): WarpEnvelope {
  const [[x0, y0], [x1, y1]] = bounds;
  const line = (a: Point, z: Point): Edge => [
    a,
    [a[0] + (z[0] - a[0]) / 3, a[1] + (z[1] - a[1]) / 3],
    [a[0] + (2 * (z[0] - a[0])) / 3, a[1] + (2 * (z[1] - a[1])) / 3],
    z,
  ];
  return {
    bottom: line([x0, y0], [x1, y0]),
    top: line([x0, y1], [x1, y1]),
    left: line([x0, y0], [x0, y1]),
    right: line([x1, y0], [x1, y1]),
  };
}
