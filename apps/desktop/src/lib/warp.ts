import type { Point, WarpEnvelope } from '@/api/types';
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

/** Función de deformación de la región `bounds` a la envolvente, o `null` si la región no tiene ancho y alto. */
export function makeWarp(bounds: Bounds, envelope: WarpEnvelope): ((p: Point) => Point) | null {
  const w = bounds[1][0] - bounds[0][0];
  const h = bounds[1][1] - bounds[0][1];
  if (!(w > 1e-9) || !(h > 1e-9)) return null;
  const env = snapEnvelope(envelope);
  return ([x, y]) => envelopeAt(env, (x - bounds[0][0]) / w, (y - bounds[0][1]) / h);
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
