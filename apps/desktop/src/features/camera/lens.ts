/**
 * Corrección de lente en el navegador. El modelo es el de `tlaku-camera` (Brown-Conrady con
 * k1, k2, k3, p1, p2): para cada píxel de la imagen corregida se calcula dónde cae en la imagen
 * cruda y se muestrea ahí. `correctedToRaw` es la versión de referencia en TS del shader.
 */
import type { LensCalibration } from '@/api/types';

export type LensParams = Pick<LensCalibration, 'width' | 'height' | 'fx' | 'fy' | 'cx' | 'cy' | 'k1' | 'k2' | 'p1' | 'p2' | 'k3'>;

/** Coordenadas normalizadas sin distorsión -> con distorsión. */
export function distortNormalized(c: LensParams, x: number, y: number): [number, number] {
  const r2 = x * x + y * y;
  const q = 1 + r2 * (c.k1 + r2 * (c.k2 + r2 * c.k3));
  return [x * q + 2 * c.p1 * x * y + c.p2 * (r2 + 2 * x * x), y * q + c.p1 * (r2 + 2 * y * y) + 2 * c.p2 * x * y];
}

/** Píxel (u, v) de la imagen corregida -> píxel de la imagen cruda. Mismo cálculo que el shader. */
export function correctedToRaw(c: LensParams, u: number, v: number): [number, number] {
  const [xd, yd] = distortNormalized(c, (u - c.cx) / c.fx, (v - c.cy) / c.fy);
  return [c.fx * xd + c.cx, c.fy * yd + c.cy];
}

/** ¿El píxel crudo cae dentro de la imagen? (los bordes de la imagen corregida quedan transparentes) */
export function insideRaw(c: LensParams, x: number, y: number): boolean {
  return x >= 0 && y >= 0 && x <= c.width - 1 && y <= c.height - 1;
}

/** Tolerancia de proporción entre el video y la calibración para escalar los parámetros. */
const ASPECT_TOLERANCE = 0.01;

/** La calibración sirve para un video de `w` x `h` si tiene la misma proporción (se escala). */
export function lensFitsStream(c: LensParams, w: number, h: number): boolean {
  if (w <= 0 || h <= 0) return false;
  return Math.abs(w / h - c.width / c.height) / (c.width / c.height) <= ASPECT_TOLERANCE;
}

/** Tamaño de la imagen corregida que se pinta: el de la calibración, acotado a `maxWidth`. */
export function correctedSize(c: LensParams, maxWidth = 1280): [number, number] {
  const s = Math.min(1, maxWidth / c.width);
  return [Math.max(2, Math.round(c.width * s)), Math.max(2, Math.round(c.height * s))];
}

/** Valores de los uniformes del shader (todo en píxeles de la calibración). */
export interface LensUniforms {
  size: [number, number];
  k: [number, number, number, number];
  rad: [number, number, number];
  tan: [number, number];
}

export function lensUniforms(c: LensParams): LensUniforms {
  return { size: [c.width, c.height], k: [c.fx, c.fy, c.cx, c.cy], rad: [c.k1, c.k2, c.k3], tan: [c.p1, c.p2] };
}

export const VERTEX_SHADER = `attribute vec2 aPos;
varying vec2 vUv;
void main() {
  // vUv: (0,0) arriba a la izquierda de la imagen, como las filas del video.
  vUv = vec2(aPos.x * 0.5 + 0.5, 0.5 - aPos.y * 0.5);
  gl_Position = vec4(aPos, 0.0, 1.0);
}`;

/** Brown-Conrady, igual que \`distortNormalized\` / \`correctedToRaw\` de lens.ts. */
export const FRAGMENT_SHADER = `precision highp float;
varying vec2 vUv;
uniform sampler2D uTex;
uniform vec2 uSize;
uniform vec4 uK;
uniform vec3 uRad;
uniform vec2 uTan;
uniform float uCorrect;
void main() {
  vec2 uv = vUv;
  if (uCorrect > 0.5) {
    vec2 px = vUv * uSize - 0.5;
    vec2 n = (px - uK.zw) / uK.xy;
    float r2 = dot(n, n);
    float q = 1.0 + r2 * (uRad.x + r2 * (uRad.y + r2 * uRad.z));
    vec2 d = vec2(
      n.x * q + 2.0 * uTan.x * n.x * n.y + uTan.y * (r2 + 2.0 * n.x * n.x),
      n.y * q + uTan.x * (r2 + 2.0 * n.y * n.y) + 2.0 * uTan.y * n.x * n.y);
    uv = (d * uK.xy + uK.zw + 0.5) / uSize;
    vec2 lo = 0.5 / uSize;
    vec2 hi = 1.0 - lo;
    if (any(lessThan(uv, lo)) || any(greaterThan(uv, hi))) {
      gl_FragColor = vec4(0.0);
      return;
    }
  }
  gl_FragColor = vec4(texture2D(uTex, uv).rgb, 1.0);
}`;
