/**
 * Corrección de lente en el navegador. El modelo es el de `tlaku-camera`: Brown-Conrady
 * (k1, k2, k3, p1, p2) u ojo de pez equidistante de Kannala-Brandt (k1..k4, campo `model`).
 * Para cada píxel de la imagen corregida se calcula dónde cae en la imagen cruda y se muestrea
 * ahí. `correctedToRaw` es la versión de referencia en TS del shader. La imagen corregida usa la
 * focal de la calibración por `view_zoom` (1 con Brown-Conrady; con ojo de pez es menor para que
 * quepa un campo amplio).
 */
import type { LensCalibration } from '@/api/types';

export type LensParams = Pick<LensCalibration, 'width' | 'height' | 'fx' | 'fy' | 'cx' | 'cy' | 'k1' | 'k2' | 'p1' | 'p2' | 'k3'> &
  Partial<Pick<LensCalibration, 'model' | 'k4' | 'view_zoom'>>;

/** ¿La lente es un ojo de pez? Las calibraciones guardadas antes no traen `model`: son Brown-Conrady. */
export function isFisheye(c: LensParams): boolean {
  return c.model === 'fisheye';
}

/** Factor de la focal de la imagen corregida (1 si la calibración no lo trae). */
export function viewZoom(c: LensParams): number {
  const z = c.view_zoom;
  return typeof z === 'number' && Number.isFinite(z) && z > 0 ? z : 1;
}

/** Focales de la imagen corregida. */
export function correctedFocal(c: LensParams): [number, number] {
  const z = viewZoom(c);
  return [c.fx * z, c.fy * z];
}

/** Coordenadas normalizadas sin distorsión -> con distorsión. */
export function distortNormalized(c: LensParams, x: number, y: number): [number, number] {
  if (isFisheye(c)) {
    const r = Math.hypot(x, y);
    if (r < 1e-12) return [x, y];
    const th = Math.atan(r);
    const t2 = th * th;
    const thd = th * (1 + t2 * (c.k1 + t2 * (c.k2 + t2 * (c.k3 + t2 * (c.k4 ?? 0)))));
    const s = thd / r;
    return [x * s, y * s];
  }
  const r2 = x * x + y * y;
  const q = 1 + r2 * (c.k1 + r2 * (c.k2 + r2 * c.k3));
  return [x * q + 2 * c.p1 * x * y + c.p2 * (r2 + 2 * x * x), y * q + c.p1 * (r2 + 2 * y * y) + 2 * c.p2 * x * y];
}

/** Píxel (u, v) de la imagen corregida -> píxel de la imagen cruda. Mismo cálculo que el shader. */
export function correctedToRaw(c: LensParams, u: number, v: number): [number, number] {
  const [fx, fy] = correctedFocal(c);
  const [xd, yd] = distortNormalized(c, (u - c.cx) / fx, (v - c.cy) / fy);
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
  /** k1, k2, k3, k4 (con Brown-Conrady, k4 no se usa). */
  rad: [number, number, number, number];
  tan: [number, number];
  /** Factor de la focal de la imagen corregida. */
  zoom: number;
  /** 1 = ojo de pez (Kannala-Brandt), 0 = Brown-Conrady. */
  fisheye: number;
}

export function lensUniforms(c: LensParams): LensUniforms {
  return {
    size: [c.width, c.height],
    k: [c.fx, c.fy, c.cx, c.cy],
    rad: [c.k1, c.k2, c.k3, c.k4 ?? 0],
    tan: [c.p1, c.p2],
    zoom: viewZoom(c),
    fisheye: isFisheye(c) ? 1 : 0,
  };
}

export const VERTEX_SHADER = `attribute vec2 aPos;
varying vec2 vUv;
void main() {
  // vUv: (0,0) arriba a la izquierda de la imagen, como las filas del video.
  vUv = vec2(aPos.x * 0.5 + 0.5, 0.5 - aPos.y * 0.5);
  gl_Position = vec4(aPos, 0.0, 1.0);
}`;

/**
 * Brown-Conrady u ojo de pez de Kannala-Brandt (\`uFisheye\`), igual que \`distortNormalized\` /
 * \`correctedToRaw\` de lens.ts. \`uGray\` pasa el resultado a escala de grises.
 */
export const FRAGMENT_SHADER = `precision highp float;
varying vec2 vUv;
uniform sampler2D uTex;
uniform vec2 uSize;
uniform vec4 uK;
uniform vec4 uRad;
uniform vec2 uTan;
uniform float uZoom;
uniform float uFisheye;
uniform float uGray;
uniform float uCorrect;
void main() {
  vec2 uv = vUv;
  if (uCorrect > 0.5) {
    vec2 px = vUv * uSize - 0.5;
    vec2 n = (px - uK.zw) / (uK.xy * uZoom);
    vec2 d;
    if (uFisheye > 0.5) {
      float r = length(n);
      if (r < 1e-9) {
        d = n;
      } else {
        float th = atan(r);
        float t2 = th * th;
        float thd = th * (1.0 + t2 * (uRad.x + t2 * (uRad.y + t2 * (uRad.z + t2 * uRad.w))));
        d = n * (thd / r);
      }
    } else {
      float r2 = dot(n, n);
      float q = 1.0 + r2 * (uRad.x + r2 * (uRad.y + r2 * uRad.z));
      d = vec2(
        n.x * q + 2.0 * uTan.x * n.x * n.y + uTan.y * (r2 + 2.0 * n.x * n.x),
        n.y * q + uTan.x * (r2 + 2.0 * n.y * n.y) + 2.0 * uTan.y * n.x * n.y);
    }
    uv = (d * uK.xy + uK.zw + 0.5) / uSize;
    vec2 lo = 0.5 / uSize;
    vec2 hi = 1.0 - lo;
    if (any(lessThan(uv, lo)) || any(greaterThan(uv, hi))) {
      gl_FragColor = vec4(0.0);
      return;
    }
  }
  vec3 rgb = texture2D(uTex, uv).rgb;
  if (uGray > 0.5) rgb = vec3(dot(rgb, vec3(0.299, 0.587, 0.114)));
  gl_FragColor = vec4(rgb, 1.0);
}`;
