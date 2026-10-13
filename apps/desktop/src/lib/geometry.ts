import type { Affine, DesignObject, Point } from '@/api/types';

export const IDENTITY: Affine = [1, 0, 0, 1, 0, 0];

export type Bounds = [Point, Point];

/** x' = a·x + c·y + e ; y' = b·x + d·y + f */
export function applyAffine(m: Affine, [x, y]: Point): Point {
  return [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]];
}

export function translateAffine(m: Affine, dx: number, dy: number): Affine {
  return [m[0], m[1], m[2], m[3], m[4] + dx, m[5] + dy];
}

export function translation(dx: number, dy: number): Affine {
  return [1, 0, 0, 1, dx, dy];
}

/** Polilíneas del objeto en coordenadas de documento (mm). Las imágenes dan su caja. */
export function objectPolylines(obj: DesignObject): { points: Point[]; closed: boolean }[] {
  if (obj.kind.type === 'vector') {
    return obj.kind.paths.map((p) => ({
      points: p.points.map((pt) => applyAffine(obj.transform, pt)),
      closed: p.closed,
    }));
  }
  const [w, h] = obj.kind.size_mm;
  const corners: Point[] = [
    [0, 0],
    [w, 0],
    [w, h],
    [0, h],
  ];
  return [{ points: corners.map((c) => applyAffine(obj.transform, c)), closed: true }];
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

export function objectBounds(obj: DesignObject): Bounds | null {
  return pointsBounds(objectPolylines(obj).flatMap((p) => p.points));
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

/** Formas que generan los ejemplos y el mock. Todas en mm, sentido antihorario. */
export const shapes = {
  circle(cx: number, cy: number, r: number, n = 64): Point[] {
    return Array.from({ length: n }, (_, i) => {
      const t = (i / n) * Math.PI * 2;
      return [cx + r * Math.cos(t), cy + r * Math.sin(t)] as Point;
    });
  },
  rect(x: number, y: number, w: number, h: number): Point[] {
    return [
      [x, y],
      [x + w, y],
      [x + w, y + h],
      [x, y + h],
    ];
  },
  roundedRect(x: number, y: number, w: number, h: number, r: number, seg = 8): Point[] {
    const pts: Point[] = [];
    const corners: [number, number, number][] = [
      [x + w - r, y + r, -Math.PI / 2],
      [x + w - r, y + h - r, 0],
      [x + r, y + h - r, Math.PI / 2],
      [x + r, y + r, Math.PI],
    ];
    for (const [cx, cy, a0] of corners) {
      for (let i = 0; i <= seg; i++) {
        const a = a0 + (i / seg) * (Math.PI / 2);
        pts.push([cx + r * Math.cos(a), cy + r * Math.sin(a)]);
      }
    }
    return pts;
  },
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
