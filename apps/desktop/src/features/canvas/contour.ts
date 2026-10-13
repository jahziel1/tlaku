import type { DesignObject, Point } from '@/api/types';
import { getGeom, type PolyGeom } from './geom';

/**
 * Geometría sobre el contorno de un objeto, en coordenadas de documento (mm): el punto del trazo
 * más cercano a un clic, tramos de contorno alrededor de un punto y el sentido de recorrido. Sirve
 * a las herramientas de puentes e inicio y a lo que se dibuja encima del lienzo.
 */

/** Punto del contorno más cercano a otro. */
export interface ContourHit {
  /** Punto sobre el contorno (mm). */
  point: Point;
  /** Distancia del punto consultado al contorno (mm). */
  dist: number;
  /** Subtrayecto del objeto (posición en `getGeom(obj).polys`). */
  poly: number;
  /** Longitud de arco (mm) desde el primer punto del subtrayecto hasta `point`. */
  arc: number;
  closed: boolean;
}

/** Vértices aplanados del subtrayecto como puntos. */
export function polyPoints(poly: PolyGeom): Point[] {
  const out: Point[] = [];
  for (let i = 0; i + 1 < poly.pts.length; i += 2) out.push([poly.pts[i]!, poly.pts[i + 1]!]);
  return out;
}

/** Longitud de arco acumulada hasta cada vértice; el último elemento es el perímetro (con cierre si es cerrado). */
export function cumulativeLength(pts: readonly Point[], closed: boolean): number[] {
  const cum = [0];
  const n = pts.length;
  const segs = closed ? n : n - 1;
  for (let i = 0; i < segs; i++) {
    const a = pts[i]!;
    const b = pts[(i + 1) % n]!;
    cum.push(cum[i]! + Math.hypot(b[0] - a[0], b[1] - a[1]));
  }
  return cum;
}

/** Punto más cercano a `p` en el trazo del objeto (de todos sus subtrayectos), o `null` si no hay trazo. */
export function nearestOnContour(obj: DesignObject, p: Point, onlyClosed = false): ContourHit | null {
  const g = getGeom(obj);
  let best: ContourHit | null = null;
  g.polys.forEach((poly, k) => {
    if (onlyClosed && !poly.closed) return;
    const pts = polyPoints(poly);
    const n = pts.length;
    if (n === 0) return;
    const cum = cumulativeLength(pts, poly.closed);
    const segs = n === 1 ? 0 : poly.closed ? n : n - 1;
    if (segs === 0) {
      const d = Math.hypot(p[0] - pts[0]![0], p[1] - pts[0]![1]);
      if (!best || d < best.dist) best = { point: pts[0]!, dist: d, poly: k, arc: 0, closed: poly.closed };
      return;
    }
    for (let i = 0; i < segs; i++) {
      const a = pts[i]!;
      const b = pts[(i + 1) % n]!;
      const dx = b[0] - a[0];
      const dy = b[1] - a[1];
      const len2 = dx * dx + dy * dy;
      const t = len2 === 0 ? 0 : Math.min(1, Math.max(0, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2));
      const q: Point = [a[0] + t * dx, a[1] + t * dy];
      const d = Math.hypot(p[0] - q[0], p[1] - q[1]);
      if (!best || d < best.dist) best = { point: q, dist: d, poly: k, arc: cum[i]! + t * Math.sqrt(len2), closed: poly.closed };
    }
  });
  return best;
}

/** Punto del subtrayecto en la longitud de arco `u` (se envuelve si es cerrado). */
export function pointAtArc(pts: readonly Point[], cum: readonly number[], closed: boolean, u: number): { point: Point; dir: Point } {
  const n = pts.length;
  const total = cum[cum.length - 1]!;
  if (n === 1 || total <= 0) return { point: pts[0]!, dir: [1, 0] };
  const w = closed ? ((u % total) + total) % total : Math.min(total, Math.max(0, u));
  let i = 0;
  while (i < cum.length - 2 && cum[i + 1]! <= w) i++;
  const a = pts[i]!;
  const b = pts[(i + 1) % n]!;
  const len = cum[i + 1]! - cum[i]!;
  const t = len > 0 ? (w - cum[i]!) / len : 0;
  const dir: Point = len > 0 ? [(b[0] - a[0]) / len, (b[1] - a[1]) / len] : [1, 0];
  return { point: [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t], dir };
}

/**
 * Tramo del contorno de largo `2 * half` centrado en la longitud de arco `arc`. En un contorno
 * abierto se recorta a sus extremos; en uno cerrado da la vuelta.
 */
export function spanAround(pts: readonly Point[], closed: boolean, arc: number, half: number): Point[] {
  const cum = cumulativeLength(pts, closed);
  const total = cum[cum.length - 1]!;
  if (pts.length < 2 || total <= 0) return pts.length ? [pts[0]!] : [];
  const from = closed ? arc - half : Math.max(0, arc - half);
  const to = closed ? arc + half : Math.min(total, arc + half);
  const out: Point[] = [pointAtArc(pts, cum, closed, from).point];
  const laps = closed ? 2 : 1;
  for (let lap = 0; lap < laps; lap++) {
    for (let j = 0; j < pts.length; j++) {
      const pos = cum[j]! + lap * total;
      const base = closed ? ((from % total) + total) % total : from;
      const lo = base;
      const hi = base + (to - from);
      if (pos > lo + 1e-9 && pos < hi - 1e-9) out.push(pts[j]!);
    }
  }
  out.push(pointAtArc(pts, cum, closed, to).point);
  return out;
}

/** Dónde empieza y hacia dónde va el recorrido de un contorno, según su `start` (o el trazo tal como se dibujó). */
export interface StartMark {
  point: Point;
  /** Sentido de avance, vector unitario. */
  dir: Point;
  poly: number;
  /** El inicio lo fijó el usuario (si no, es el del trazo tal como se dibujó). */
  fixed: boolean;
  reversed: boolean;
}

/**
 * Subtrayecto, punto y sentido de la flecha de inicio de un objeto. Igual que el motor: el punto se
 * lleva al contorno **cerrado** más cercano; sin punto, vale el inicio del primer subtrayecto con
 * puntos. `reverse` invierte el sentido. `null` si el objeto no tiene trazo.
 */
export function startMark(obj: DesignObject): StartMark | null {
  const g = getGeom(obj);
  const spec = obj.start ?? null;
  const reversed = spec?.reverse ?? false;
  let poly = g.polys.findIndex((p) => p.pts.length >= 2);
  let arc = 0;
  if (spec?.point) {
    const hit = nearestOnContour(obj, spec.point, true);
    if (hit) {
      poly = hit.poly;
      arc = hit.arc;
    }
  }
  if (poly < 0) return null;
  const pts = polyPoints(g.polys[poly]!);
  const closed = g.polys[poly]!.closed;
  const cum = cumulativeLength(pts, closed);
  const at = pointAtArc(pts, cum, closed, arc);
  // En el trazo abierto invertido el inicio pasa al otro extremo.
  if (!closed && reversed) {
    const end = pointAtArc(pts, cum, closed, cum[cum.length - 1]!);
    const back = pointAtArc(pts, cum, closed, Math.max(0, cum[cum.length - 1]! - 1e-6));
    return { point: end.point, dir: [-back.dir[0], -back.dir[1]], poly, fixed: spec !== null, reversed };
  }
  const dir: Point = reversed ? [-at.dir[0], -at.dir[1]] : at.dir;
  // Con sentido invertido, la dirección de avance es la del segmento anterior al punto, al revés.
  if (reversed && closed) {
    const prev = pointAtArc(pts, cum, closed, arc - 1e-6);
    return { point: at.point, dir: [-prev.dir[0], -prev.dir[1]], poly, fixed: spec !== null, reversed };
  }
  return { point: at.point, dir, poly, fixed: spec !== null, reversed };
}
