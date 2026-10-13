import type { DesignObject, LayerSettings, PathData, Point } from '@/api/types';
import { IDENTITY, type Bounds } from '@/lib/geometry';
import { LAYER_PALETTE } from '@/lib/palette';

/** Error máximo permitido al aproximar curvas con segmentos (mm). */
export const CURVE_TOLERANCE_MM = 0.02;

/** Segmentos para un arco de `sweep` radianes de radio `r` sin pasar de la tolerancia. */
export function arcSegments(r: number, sweep: number, tol = CURVE_TOLERANCE_MM): number {
  if (r <= tol) return Math.max(2, Math.ceil(sweep / (Math.PI / 4)));
  const step = 2 * Math.acos(1 - tol / r);
  return Math.max(2, Math.ceil(sweep / step));
}

export function ellipsePoints(cx: number, cy: number, rx: number, ry: number): Point[] {
  // Múltiplo de 4: hay vértices exactamente en los cuatro extremos, así la caja mide lo que se dibujó.
  const n = Math.ceil(Math.min(720, Math.max(24, arcSegments(Math.max(rx, ry), Math.PI * 2))) / 4) * 4;
  return Array.from({ length: n }, (_, i) => {
    const t = (i / n) * Math.PI * 2;
    return [cx + rx * Math.cos(t), cy + ry * Math.sin(t)] as Point;
  });
}

/** Rectángulo antihorario desde la esquina inferior izquierda; con `radius` > 0 las esquinas son arcos. */
export function rectPoints(x0: number, y0: number, x1: number, y1: number, radius = 0): Point[] {
  const minX = Math.min(x0, x1);
  const maxX = Math.max(x0, x1);
  const minY = Math.min(y0, y1);
  const maxY = Math.max(y0, y1);
  const r = Math.min(Math.max(0, radius), (maxX - minX) / 2, (maxY - minY) / 2);
  if (r <= 1e-9) {
    return [[minX, minY], [maxX, minY], [maxX, maxY], [minX, maxY]];
  }
  const seg = Math.max(2, arcSegments(r, Math.PI / 2));
  const pts: Point[] = [];
  const corners: [number, number, number][] = [
    [maxX - r, minY + r, -Math.PI / 2],
    [maxX - r, maxY - r, 0],
    [minX + r, maxY - r, Math.PI / 2],
    [minX + r, minY + r, Math.PI],
  ];
  for (const [cx, cy, a0] of corners) {
    for (let i = 0; i <= seg; i++) {
      const a = a0 + (i / seg) * (Math.PI / 2);
      pts.push([cx + r * Math.cos(a), cy + r * Math.sin(a)]);
    }
  }
  return pts;
}

export interface DragRectOptions {
  /** Mayús: cuadrado / círculo. */
  square: boolean;
  /** Alt: el punto inicial es el centro. */
  fromCenter: boolean;
}

/** Caja que resulta de arrastrar de `a` a `b`. */
export function dragBox(a: Point, b: Point, opts: DragRectOptions): Bounds {
  let dx = b[0] - a[0];
  let dy = b[1] - a[1];
  if (opts.square) {
    const s = Math.max(Math.abs(dx), Math.abs(dy));
    dx = (dx < 0 ? -1 : 1) * s;
    dy = (dy < 0 ? -1 : 1) * s;
  }
  if (opts.fromCenter) {
    return [[a[0] - Math.abs(dx), a[1] - Math.abs(dy)], [a[0] + Math.abs(dx), a[1] + Math.abs(dy)]];
  }
  return [[Math.min(a[0], a[0] + dx), Math.min(a[1], a[1] + dy)], [Math.max(a[0], a[0] + dx), Math.max(a[1], a[1] + dy)]];
}

/** Lleva `to` al múltiplo de `stepDeg` más cercano visto desde `from` (Mayús al dibujar líneas). */
export function constrainAngle(from: Point, to: Point, stepDeg = 45): Point {
  const dx = to[0] - from[0];
  const dy = to[1] - from[1];
  const len = Math.hypot(dx, dy);
  if (len === 0) return to;
  const step = (stepDeg * Math.PI) / 180;
  const a = Math.round(Math.atan2(dy, dx) / step) * step;
  return [from[0] + len * Math.cos(a), from[1] + len * Math.sin(a)];
}

/** Al mover con Mayús: deja solo el eje dominante (el movimiento queda horizontal o vertical). */
export function constrainDelta(dx: number, dy: number): [number, number] {
  return Math.abs(dx) >= Math.abs(dy) ? [dx, 0] : [0, dy];
}

/** Capa donde se dibuja: la activa, o la primera de líneas/relleno, o el primer color de la paleta. */
export function pickDrawLayer(layers: readonly LayerSettings[], activeLayer: string | null): string {
  const usable = (l: LayerSettings) => l.mode !== 'image';
  if (activeLayer) {
    const l = layers.find((x) => x.color.toLowerCase() === activeLayer.toLowerCase());
    if (l && usable(l)) return l.color;
    if (!l) return activeLayer;
  }
  const first = layers.find(usable);
  if (first) return first.color;
  const free = LAYER_PALETTE.find((p) => !p.guide && !layers.some((l) => l.color.toLowerCase() === p.color.toLowerCase()));
  return (free ?? LAYER_PALETTE[0]).color;
}

export function newVectorObject(id: string, layer: string, paths: PathData[]): DesignObject {
  return { id, layer, transform: [...IDENTITY], kind: { type: 'vector', paths }, locked: false, hidden: false };
}

export type ShapeKind = 'rect' | 'ellipse' | 'line' | 'polyline';

/** Rutas de la forma definida por una caja de arrastre. */
export function boxShapePaths(kind: 'rect' | 'ellipse', box: Bounds, radius = 0): PathData[] {
  if (kind === 'rect') return [{ points: rectPoints(box[0][0], box[0][1], box[1][0], box[1][1], radius), closed: true }];
  const rx = (box[1][0] - box[0][0]) / 2;
  const ry = (box[1][1] - box[0][1]) / 2;
  return [{ points: ellipsePoints(box[0][0] + rx, box[0][1] + ry, rx, ry), closed: true }];
}

/** Puntos de una polilínea sin repetir vecinos iguales (el doble clic suma un punto duplicado). */
export function dedupePoints(points: readonly Point[], eps = 1e-9): Point[] {
  const out: Point[] = [];
  for (const p of points) {
    const last = out[out.length - 1];
    if (!last || Math.abs(last[0] - p[0]) > eps || Math.abs(last[1] - p[1]) > eps) out.push(p);
  }
  return out;
}

export function polylinePaths(points: readonly Point[], close: boolean): PathData[] | null {
  const pts = dedupePoints(points);
  if (pts.length < 2) return null;
  return [{ points: pts, closed: close && pts.length >= 3 }];
}
