import type { Point } from '@/api/types';
import type { TileSpec } from './plan';

/** Movimiento del simulador: `[tipo (0 = viaje, 1 = corte), x, y, potencia]`. */
export type Move = [kind: 0 | 1, x: number, y: number, power: number];

const inside = (r: TileSpec['rect'], x: number, y: number) => x >= r[0][0] && x <= r[1][0] && y >= r[0][1] && y <= r[1][1];

/** Parte del segmento a-b dentro de `r` (Liang-Barsky): parámetros `[t0, t1]`, o `null` si no toca. */
export function clipSegment(r: TileSpec['rect'], a: Point, b: Point): [number, number] | null {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  let t0 = 0;
  let t1 = 1;
  const edges: [number, number][] = [
    [-dx, a[0] - r[0][0]],
    [dx, r[1][0] - a[0]],
    [-dy, a[1] - r[0][1]],
    [dy, r[1][1] - a[1]],
  ];
  for (const [p, q] of edges) {
    if (p === 0) {
      if (q < 0) return null;
    } else {
      const t = q / p;
      if (p < 0) t0 = Math.max(t0, t);
      else t1 = Math.min(t1, t);
    }
  }
  return t0 <= t1 ? [t0, t1] : null;
}

/**
 * Recorta los movimientos del simulador al rectángulo del tramo y los desplaza `offset`: los
 * tramos de corte se parten en el borde (con un viaje hasta cada pieza) y los viajes se
 * descartan fuera. Es el equivalente simple de `tile::apply` del motor, que recorta la geometría
 * antes de planificar.
 */
export function clipMoves(moves: Move[], tile: TileSpec): Move[] {
  const [dx, dy] = tile.offset;
  const out: Move[] = [];
  let prev: Point | null = null;
  // Última posición emitida: si la pieza siguiente empieza en otro lado, hay que viajar hasta ahí.
  let emitted: Point | null = null;
  // Se ajusta al rectángulo antes de desplazar: el redondeo no debe dejar un punto 1e-15 fuera.
  const shift = (x: number, y: number): [number, number] => [
    Math.min(tile.rect[1][0], Math.max(tile.rect[0][0], x)) + dx,
    Math.min(tile.rect[1][1], Math.max(tile.rect[0][1], y)) + dy,
  ];
  for (const m of moves) {
    const to: Point = [m[1], m[2]];
    if (m[0] === 0 || prev === null) {
      prev = to;
      continue;
    }
    const clipped = clipSegment(tile.rect, prev, to);
    if (clipped) {
      const [t0, t1] = clipped;
      const p0: Point = [prev[0] + (to[0] - prev[0]) * t0, prev[1] + (to[1] - prev[1]) * t0];
      const p1: Point = [prev[0] + (to[0] - prev[0]) * t1, prev[1] + (to[1] - prev[1]) * t1];
      if (!emitted || Math.hypot(emitted[0] - p0[0], emitted[1] - p0[1]) > 1e-9) {
        out.push([0, ...shift(p0[0], p0[1]), 0]);
      }
      out.push([1, ...shift(p1[0], p1[1]), m[3]]);
      emitted = p1;
    }
    prev = to;
  }
  return out;
}

/** ¿El punto cae dentro del rectángulo del tramo? */
export function inTile(tile: TileSpec, p: Point): boolean {
  return inside(tile.rect, p[0], p[1]);
}
