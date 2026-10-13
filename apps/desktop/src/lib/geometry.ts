import type { Affine, DesignObject, Point } from '@/api/types';
import { bezBounds, FLATTEN_TOLERANCE_MM, flattenBez, polylineBez, transformBez, type BezPath } from './bezpath';
import { textPlaceholderBez } from './textobj';

export const IDENTITY: Affine = [1, 0, 0, 1, 0, 0];

export type Bounds = [Point, Point];

/** x' = a·x + c·y + e ; y' = b·x + d·y + f */
export function applyAffine(m: Affine, [x, y]: Point): Point {
  return [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]];
}

export function translateAffine(m: Affine, dx: number, dy: number): Affine {
  return [m[0], m[1], m[2], m[3], m[4] + dx, m[5] + dy];
}

/** Composición: primero `inner`, después `outer`. */
export function multiplyAffine(outer: Affine, inner: Affine): Affine {
  const [a, b, c, d, e, f] = outer;
  const [a2, b2, c2, d2, e2, f2] = inner;
  return [
    a * a2 + c * b2,
    b * a2 + d * b2,
    a * c2 + c * d2,
    b * c2 + d * d2,
    a * e2 + c * f2 + e,
    b * e2 + d * f2 + f,
  ];
}

export function translation(dx: number, dy: number): Affine {
  return [1, 0, 0, 1, dx, dy];
}

/**
 * Curvas del objeto en coordenadas locales: las del vector, el contorno del texto (o su caja
 * estimada mientras no tenga contorno) o el rectángulo de la imagen.
 */
export function objectLocalPaths(obj: DesignObject): readonly BezPath[] {
  const kind = obj.kind;
  switch (kind.type) {
    case 'vector':
      return kind.paths;
    case 'text':
      return kind.outline.length > 0 ? kind.outline : [textPlaceholderBez(kind)];
    case 'image': {
      const [w, h] = kind.size_mm;
      return [polylineBez([[0, 0], [w, 0], [w, h], [0, h]], true)];
    }
  }
}

/** Curvas del objeto en coordenadas de documento (con el `transform` aplicado). */
export function objectCurves(obj: DesignObject): BezPath[] {
  return objectLocalPaths(obj).map((p) => transformBez(p, obj.transform));
}

/** Polilíneas del objeto en coordenadas de documento (mm), con las curvas aplanadas. */
export function objectPolylines(obj: DesignObject, tol = FLATTEN_TOLERANCE_MM): { points: Point[]; closed: boolean }[] {
  return objectCurves(obj).flatMap((p) => flattenBez(p, tol));
}

export function pointsBounds(points: Point[]): Bounds | null {
  if (points.length === 0) return null;
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const [x, y] of points) {
    if (x < minX) minX = x;
    if (y < minY) minY = y;
    if (x > maxX) maxX = x;
    if (y > maxY) maxY = y;
  }
  return [
    [minX, minY],
    [maxX, maxY],
  ];
}

/** Caja exacta del objeto (extremos de las curvas, no de sus puntos de control). */
export function objectBounds(obj: DesignObject): Bounds | null {
  return bezBounds(objectCurves(obj));
}

export function unionBounds(list: (Bounds | null)[]): Bounds | null {
  const all = list.filter((b): b is Bounds => b !== null).flatMap((b) => [b[0], b[1]]);
  return pointsBounds(all);
}

export function polylineLength(points: Point[], closed: boolean): number {
  let total = 0;
  for (let i = 1; i < points.length; i++) {
    total += Math.hypot(points[i]![0] - points[i - 1]![0], points[i]![1] - points[i - 1]![1]);
  }
  if (closed && points.length > 2) {
    const a = points[points.length - 1]!;
    const b = points[0]!;
    total += Math.hypot(a[0] - b[0], a[1] - b[1]);
  }
  return total;
}

/** Polígonos que generan los ejemplos y el mock (círculos y rectángulos son curvas: ver `lib/bezpath`). */
export const shapes = {
  star(cx: number, cy: number, rOuter: number, rInner: number, points = 5): Point[] {
    return Array.from({ length: points * 2 }, (_, i) => {
      const r = i % 2 === 0 ? rOuter : rInner;
      const t = Math.PI / 2 + (i / (points * 2)) * Math.PI * 2;
      return [cx + r * Math.cos(t), cy + r * Math.sin(t)] as Point;
    });
  },
  gear(cx: number, cy: number, rOuter: number, rRoot: number, teeth: number): Point[] {
    const pts: Point[] = [];
    const step = (Math.PI * 2) / teeth;
    for (let i = 0; i < teeth; i++) {
      const a = i * step;
      const k = [0, 0.18, 0.4, 0.58];
      const radii = [rRoot, rOuter, rOuter, rRoot];
      k.forEach((f, j) => {
        const t = a + f * step;
        pts.push([cx + radii[j]! * Math.cos(t), cy + radii[j]! * Math.sin(t)]);
      });
    }
    return pts;
  },
};

// ---- reflejos y semejanzas (voltear, espejo, rotar/escalar por dos puntos) -----------------------

/** Reflejo izquierda-derecha sobre la vertical x = `cx` (voltear horizontal). */
export function flipHorizontalAbout(cx: number): Affine {
  return [-1, 0, 0, 1, 2 * cx, 0];
}

/** Reflejo arriba-abajo sobre la horizontal y = `cy` (voltear vertical). */
export function flipVerticalAbout(cy: number): Affine {
  return [1, 0, 0, -1, 0, 2 * cy];
}

/**
 * Reflejo sobre la recta que pasa por `a` y `b`. `null` si los puntos coinciden (no hay recta).
 * Con θ el ángulo de la recta: la parte lineal es [cos 2θ, sin 2θ; sin 2θ, -cos 2θ].
 */
export function reflectAcrossLine(a: Point, b: Point): Affine | null {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const len2 = dx * dx + dy * dy;
  if (len2 < 1e-18) return null;
  const c = (dx * dx - dy * dy) / len2;
  const s = (2 * dx * dy) / len2;
  // p' = a + R (p - a)  ->  traslación = a - R a
  return [c, s, s, -c, a[0] - (c * a[0] + s * a[1]), a[1] - (s * a[0] + -c * a[1])];
}

/**
 * Giro (y, si `scale`, también escala) alrededor de `pivot` que lleva el punto `from` hasta la
 * dirección de `to`. Con `scale` la distancia pivote-`from` pasa a ser la de pivote-`to`.
 * `null` si algún punto coincide con el pivote.
 */
export function twoPointSimilarity(pivot: Point, from: Point, to: Point, scale: boolean): Affine | null {
  const ax = from[0] - pivot[0];
  const ay = from[1] - pivot[1];
  const bx = to[0] - pivot[0];
  const by = to[1] - pivot[1];
  const la = Math.hypot(ax, ay);
  const lb = Math.hypot(bx, by);
  if (la < 1e-9 || lb < 1e-9) return null;
  const rot = Math.atan2(by, bx) - Math.atan2(ay, ax);
  const k = scale ? lb / la : 1;
  const c = Math.cos(rot) * k;
  const s = Math.sin(rot) * k;
  return [c, s, -s, c, pivot[0] - (c * pivot[0] - s * pivot[1]), pivot[1] - (s * pivot[0] + c * pivot[1])];
}

/**
 * Qué hace una matriz sobre lo que se transforma, para los interruptores de transformación:
 * `move` (solo traslación), `rotate` (giro puro, con o sin traslación) o `scale` (todo lo demás:
 * escala, reflejo o sesgo).
 */
export function transformKind(m: Affine, eps = 1e-9): 'move' | 'rotate' | 'scale' {
  const [a, b, c, d] = m;
  if (Math.abs(a - 1) < eps && Math.abs(b) < eps && Math.abs(c) < eps && Math.abs(d - 1) < eps) return 'move';
  const orthonormal = Math.abs(a * a + b * b - 1) < 1e-7 && Math.abs(c * c + d * d - 1) < 1e-7 && Math.abs(a * c + b * d) < 1e-7;
  return orthonormal && a * d - b * c > 0 ? 'rotate' : 'scale';
}
