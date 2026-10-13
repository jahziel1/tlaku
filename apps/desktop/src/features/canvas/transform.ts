import type { Affine, DesignObject, Point } from '@/api/types';
import type { Bounds } from '@/lib/geometry';
import { getGeom, type Box } from './geom';

/** Punto de referencia de 9 posiciones, fila por fila desde arriba a la izquierda (como `job_origin`). */
export type Anchor = number;
export const ANCHOR_CENTER: Anchor = 4;
/** Abajo a la izquierda: coincide con el origen del documento. */
export const ANCHOR_DEFAULT: Anchor = 6;

const EPS = 1e-9;
const MIN_SCALE = 1e-3;

export type HandleId = 'nw' | 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w';
export const HANDLES: readonly HandleId[] = ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w'];

/** Fracción (0 = mínimo, 0.5 = medio, 1 = máximo) de cada manija sobre la caja, con Y hacia arriba. */
const HANDLE_FRACTION: Record<HandleId, Point> = {
  nw: [0, 1], n: [0.5, 1], ne: [1, 1], e: [1, 0.5],
  se: [1, 0], s: [0.5, 0], sw: [0, 0], w: [0, 0.5],
};
const OPPOSITE: Record<HandleId, HandleId> = { nw: 'se', n: 's', ne: 'sw', e: 'w', se: 'nw', s: 'n', sw: 'ne', w: 'e' };

export function oppositeHandle(h: HandleId): HandleId {
  return OPPOSITE[h];
}

export function isCornerHandle(h: HandleId): boolean {
  return h.length === 2;
}

export function boundsSize(b: Bounds): Point {
  return [b[1][0] - b[0][0], b[1][1] - b[0][1]];
}

export function boundsCenter(b: Bounds): Point {
  return [(b[0][0] + b[1][0]) / 2, (b[0][1] + b[1][1]) / 2];
}

export function handlePoint(b: Bounds, h: HandleId): Point {
  const [fx, fy] = HANDLE_FRACTION[h];
  return [b[0][0] + (b[1][0] - b[0][0]) * fx, b[0][1] + (b[1][1] - b[0][1]) * fy];
}

/** Punto de la caja que corresponde a una de las 9 posiciones (Y hacia arriba: la fila 0 es el borde superior). */
export function anchorPoint(b: Bounds, anchor: Anchor): Point {
  const col = anchor % 3;
  const row = Math.floor(anchor / 3);
  const x = col === 0 ? b[0][0] : col === 1 ? (b[0][0] + b[1][0]) / 2 : b[1][0];
  const y = row === 0 ? b[1][1] : row === 1 ? (b[0][1] + b[1][1]) / 2 : b[0][1];
  return [x, y];
}

// ---- matrices -----------------------------------------------------------------------------------

export function scaleAbout(origin: Point, sx: number, sy: number): Affine {
  return [sx, 0, 0, sy, origin[0] * (1 - sx), origin[1] * (1 - sy)];
}

/** Giro antihorario (Y hacia arriba) de `rad` radianes alrededor de `center`. */
export function rotateAbout(center: Point, rad: number): Affine {
  const cos = Math.cos(rad);
  const sin = Math.sin(rad);
  return [cos, sin, -sin, cos, center[0] - cos * center[0] + sin * center[1], center[1] - sin * center[0] - cos * center[1]];
}

export function translateBy(dx: number, dy: number): Affine {
  return [1, 0, 0, 1, dx, dy];
}

export function isIdentity(m: Affine, eps = 1e-9): boolean {
  return (
    Math.abs(m[0] - 1) < eps && Math.abs(m[1]) < eps && Math.abs(m[2]) < eps &&
    Math.abs(m[3] - 1) < eps && Math.abs(m[4]) < eps && Math.abs(m[5]) < eps
  );
}

export function isTranslation(m: Affine, eps = 1e-12): boolean {
  return Math.abs(m[0] - 1) < eps && Math.abs(m[1]) < eps && Math.abs(m[2]) < eps && Math.abs(m[3] - 1) < eps;
}

/** ¿Conserva los ejes (solo escala y traslación)? Entonces la caja se transforma sin recorrer puntos. */
export function keepsAxes(m: Affine): boolean {
  return Math.abs(m[1]) < 1e-12 && Math.abs(m[2]) < 1e-12;
}

function clampScale(s: number): number {
  if (!Number.isFinite(s)) return 1;
  if (Math.abs(s) >= MIN_SCALE) return s;
  return s < 0 ? -MIN_SCALE : MIN_SCALE;
}

export interface ScaleOptions {
  /** Esquinas: mantener proporción (por defecto en el lienzo; con Mayús se suelta). */
  proportional: boolean;
  /** Alt: escalar desde el centro en lugar de desde el lado opuesto. */
  fromCenter: boolean;
}

/**
 * Matriz de escala al arrastrar una manija hasta `pointer` (mm). El ancla es la manija opuesta
 * (o el centro con `fromCenter`). Las manijas de lado solo escalan un eje.
 */
export function scaleByHandle(b: Bounds, handle: HandleId, pointer: Point, opts: ScaleOptions): Affine {
  const hp = handlePoint(b, handle);
  const anchor = opts.fromCenter ? boundsCenter(b) : handlePoint(b, oppositeHandle(handle));
  const [fx, fy] = HANDLE_FRACTION[handle];
  const useX = fx !== 0.5;
  const useY = fy !== 0.5;
  const dx0 = hp[0] - anchor[0];
  const dy0 = hp[1] - anchor[1];
  const okX = useX && Math.abs(dx0) > EPS;
  const okY = useY && Math.abs(dy0) > EPS;
  let sx = okX ? (pointer[0] - anchor[0]) / dx0 : 1;
  let sy = okY ? (pointer[1] - anchor[1]) / dy0 : 1;
  if (isCornerHandle(handle) && opts.proportional) {
    const s = okX && okY ? (Math.abs(sx) >= Math.abs(sy) ? sx : sy) : okX ? sx : okY ? sy : 1;
    sx = s;
    sy = s;
  }
  return scaleAbout(anchor, clampScale(sx), clampScale(sy));
}

export interface RotateResult {
  matrix: Affine;
  /** Ángulo aplicado, en grados (antihorario). */
  degrees: number;
}

function normalizeRad(a: number): number {
  let r = a % (Math.PI * 2);
  if (r > Math.PI) r -= Math.PI * 2;
  if (r <= -Math.PI) r += Math.PI * 2;
  return r;
}

/**
 * Giro por arrastre: el ángulo entre `start` y `pointer` visto desde `center`. Con `stepDeg` el
 * resultado final (`baseDeg` + giro) cae en múltiplos de ese paso.
 */
export function rotateByPointer(center: Point, start: Point, pointer: Point, opts: { stepDeg?: number; baseDeg?: number } = {}): RotateResult {
  const a0 = Math.atan2(start[1] - center[1], start[0] - center[0]);
  const a1 = Math.atan2(pointer[1] - center[1], pointer[0] - center[0]);
  let delta = normalizeRad(a1 - a0);
  if (opts.stepDeg) {
    const base = opts.baseDeg ?? 0;
    const target = Math.round((base + (delta * 180) / Math.PI) / opts.stepDeg) * opts.stepDeg;
    delta = normalizeRad(((target - base) * Math.PI) / 180);
  }
  return { matrix: rotateAbout(center, delta), degrees: (delta * 180) / Math.PI };
}

// ---- cajas bajo una matriz -------------------------------------------------------------------------

export function toBounds(box: Box): Bounds {
  return [[box[0], box[1]], [box[2], box[3]]];
}

export function applyToPoint(m: Affine, [x, y]: Point): Point {
  return [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]];
}

/** Caja de la geometría de `objs` tras aplicar `m` (exacta; recorre puntos solo si hay giro). */
export function boundsOf(objs: readonly DesignObject[], m: Affine | null = null): Bounds | null {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  const fast = !m || keepsAxes(m);
  for (const obj of objs) {
    const g = getGeom(obj);
    if (!g.box) continue;
    if (fast) {
      const [x0, y0] = m ? applyToPoint(m, [g.box[0], g.box[1]]) : [g.box[0], g.box[1]];
      const [x1, y1] = m ? applyToPoint(m, [g.box[2], g.box[3]]) : [g.box[2], g.box[3]];
      minX = Math.min(minX, x0, x1);
      maxX = Math.max(maxX, x0, x1);
      minY = Math.min(minY, y0, y1);
      maxY = Math.max(maxY, y0, y1);
    } else {
      for (const poly of g.polys) {
        for (let i = 0; i < poly.pts.length; i += 2) {
          const x = m![0] * poly.pts[i] + m![2] * poly.pts[i + 1] + m![4];
          const y = m![1] * poly.pts[i] + m![3] * poly.pts[i + 1] + m![5];
          if (x < minX) minX = x;
          if (x > maxX) maxX = x;
          if (y < minY) minY = y;
          if (y > maxY) maxY = y;
        }
      }
    }
  }
  return minX === Infinity ? null : [[minX, minY], [maxX, maxY]];
}

/** Rotación (grados, antihoraria) de un objeto con matriz de giro y escala; 0 si no es rotable. */
export function rotationOf(obj: DesignObject): number {
  if (obj.kind.type === 'image') return 0;
  const [a, b] = obj.transform;
  if (Math.abs(a) < EPS && Math.abs(b) < EPS) return 0;
  const deg = (Math.atan2(b, a) * 180) / Math.PI;
  return Math.abs(deg) < 1e-9 ? 0 : deg;
}

/** Rotación que muestra la barra: la del objeto si es único; 0 para varios. */
export function selectionRotation(objs: readonly DesignObject[]): number {
  return objs.length === 1 ? rotationOf(objs[0]) : 0;
}

export function canRotate(objs: readonly DesignObject[]): boolean {
  return objs.length > 0 && objs.every((o) => o.kind.type !== 'image');
}

// ---- edición numérica -------------------------------------------------------------------------------

export type NumericField = 'x' | 'y' | 'w' | 'h' | 'rot';

export interface FrameValues {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Valores que muestra la barra: posición del ancla elegida y tamaño de la caja. */
export function frameValues(b: Bounds, anchor: Anchor): FrameValues {
  const [x, y] = anchorPoint(b, anchor);
  const [w, h] = boundsSize(b);
  return { x, y, w, h };
}

export interface NumericContext {
  bounds: Bounds;
  anchor: Anchor;
  lockRatio: boolean;
  /** Rotación actual en grados. */
  rotation: number;
}

/**
 * Matriz que lleva la selección al valor pedido en un campo de la barra, o `null` si no hay
 * cambio (o es imposible: ancho 0, tamaño no positivo). La escala y el giro usan el ancla elegida.
 */
export function numericEdit(field: NumericField, value: number, ctx: NumericContext): Affine | null {
  if (!Number.isFinite(value)) return null;
  const origin = anchorPoint(ctx.bounds, ctx.anchor);
  const [w, h] = boundsSize(ctx.bounds);
  let m: Affine | null = null;
  switch (field) {
    case 'x':
      m = translateBy(value - origin[0], 0);
      break;
    case 'y':
      m = translateBy(0, value - origin[1]);
      break;
    case 'w': {
      if (value <= 0 || w < EPS) return null;
      const s = value / w;
      m = scaleAbout(origin, s, ctx.lockRatio && h >= EPS ? s : 1);
      break;
    }
    case 'h': {
      if (value <= 0 || h < EPS) return null;
      const s = value / h;
      m = scaleAbout(origin, ctx.lockRatio && w >= EPS ? s : 1, s);
      break;
    }
    case 'rot':
      m = rotateAbout(origin, ((value - ctx.rotation) * Math.PI) / 180);
      break;
  }
  return isIdentity(m) ? null : m;
}
