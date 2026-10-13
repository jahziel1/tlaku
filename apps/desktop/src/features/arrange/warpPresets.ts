import type { EnvelopeEdge, Point, WarpEnvelope } from '@/api/types';
import type { Bounds } from '@/lib/geometry';

/**
 * Parámetros de la deformación (Deformar): cada una de las cuatro esquinas se desplaza `dx, dy` mm
 * y cada borde se curva `bulge` mm en su punto medio (positivo = hacia afuera). Con eso se arman
 * la envolvente de cuatro bordes Bézier (2x2) y todos los presets.
 */
export interface WarpParams {
  /** Desplazamiento de las esquinas, en orden: abajo-izquierda, abajo-derecha, arriba-derecha, arriba-izquierda. */
  corners: [Point, Point, Point, Point];
  /** Abombado de los bordes, en orden: abajo, derecha, arriba, izquierda. */
  bulge: [number, number, number, number];
}

export type WarpPreset = 'archUp' | 'archDown' | 'bulge' | 'pinch' | 'perspective' | 'slant';

export const WARP_PRESETS: readonly WarpPreset[] = ['archUp', 'archDown', 'bulge', 'pinch', 'perspective', 'slant'];

/** Intensidad (en %) con la que se propone cada preset. */
export const DEFAULT_WARP_AMOUNT = 30;

export const NO_WARP: WarpParams = { corners: [[0, 0], [0, 0], [0, 0], [0, 0]], bulge: [0, 0, 0, 0] };

/** `true` si los parámetros no deforman nada. */
export function isNoWarp(p: WarpParams): boolean {
  return p.corners.every(([x, y]) => x === 0 && y === 0) && p.bulge.every((b) => b === 0);
}

/**
 * Parámetros de un preset para la caja `bounds`. `amount` va en % (0 a 100): arco y abombado miden
 * la flecha como ese % de la mitad del alto (o del ancho en los lados); la perspectiva estrecha el
 * borde de arriba ese % de la mitad del ancho; el sesgo desplaza el borde de arriba ese % del ancho.
 */
export function presetParams(preset: WarpPreset, amount: number, bounds: Bounds): WarpParams {
  const a = Math.max(0, Math.min(100, amount)) / 100;
  const w = bounds[1][0] - bounds[0][0];
  const h = bounds[1][1] - bounds[0][1];
  const r3 = (n: number) => Math.round(n * 1000) / 1000;
  switch (preset) {
    case 'archUp': {
      const s = r3((a * h) / 2);
      return { corners: NO_WARP.corners, bulge: [-s, 0, s, 0] };
    }
    case 'archDown': {
      const s = r3((a * h) / 2);
      return { corners: NO_WARP.corners, bulge: [s, 0, -s, 0] };
    }
    case 'bulge':
    case 'pinch': {
      const sign = preset === 'bulge' ? 1 : -1;
      const sh = r3((sign * a * h) / 4);
      const sw = r3((sign * a * w) / 4);
      return { corners: NO_WARP.corners, bulge: [sh, sw, sh, sw] };
    }
    case 'perspective': {
      const inset = r3((Math.min(a, 0.9) * w) / 2);
      return { corners: [[0, 0], [0, 0], [-inset, 0], [inset, 0]], bulge: [0, 0, 0, 0] };
    }
    case 'slant': {
      const shift = r3(a * w);
      return { corners: [[0, 0], [0, 0], [shift, 0], [shift, 0]], bulge: [0, 0, 0, 0] };
    }
  }
}

/** Borde de `a` a `z` con su punto medio desplazado `bulge` mm hacia `normal` (vector unitario). */
function edgeOf(a: Point, z: Point, bulge: number, normal: Point): EnvelopeEdge {
  const k = (4 / 3) * bulge;
  const at = (t: number): Point => [a[0] + (z[0] - a[0]) * t + normal[0] * k, a[1] + (z[1] - a[1]) * t + normal[1] * k];
  return [a, at(1 / 3), at(2 / 3), z];
}

/** Normal hacia afuera de un borde que va de `a` a `z`; `side` = -1 gira a la derecha y 1 a la izquierda. */
function outward(a: Point, z: Point, side: 1 | -1): Point {
  const dx = z[0] - a[0];
  const dy = z[1] - a[1];
  const len = Math.hypot(dx, dy);
  if (len < 1e-12) return [0, 0];
  return side === 1 ? [-dy / len, dx / len] : [dy / len, -dx / len];
}

/** Envolvente de cuatro bordes Bézier que corresponde a los parámetros sobre la caja `bounds`. */
export function envelopeOf(p: WarpParams, bounds: Bounds): WarpEnvelope {
  const [[x0, y0], [x1, y1]] = bounds;
  const bl: Point = [x0 + p.corners[0][0], y0 + p.corners[0][1]];
  const br: Point = [x1 + p.corners[1][0], y0 + p.corners[1][1]];
  const tr: Point = [x1 + p.corners[2][0], y1 + p.corners[2][1]];
  const tl: Point = [x0 + p.corners[3][0], y1 + p.corners[3][1]];
  const [bottom, right, top, left] = p.bulge;
  return {
    // Los bordes de abajo y de la derecha van con el exterior a su derecha; los de arriba y la izquierda, a su izquierda.
    bottom: edgeOf(bl, br, bottom, outward(bl, br, -1)),
    top: edgeOf(tl, tr, top, outward(tl, tr, 1)),
    left: edgeOf(bl, tl, left, outward(bl, tl, 1)),
    right: edgeOf(br, tr, right, outward(br, tr, -1)),
  };
}
