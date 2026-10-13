import type { DesignObject } from '@/api/types';

/** Caja [minX, minY, maxX, maxY] en mm. Plana y sin asignaciones: se usa en rutas calientes. */
export type Box = [number, number, number, number];

/** Puntos por trozo de trazo: trozos pequeños permiten descartar lo que queda fuera de la vista. */
export const CHUNK_SEGMENTS = 1024;

/** Contorno en coordenadas de documento (ya con el `transform` del objeto aplicado). */
export interface PolyGeom {
  /** x0, y0, x1, y1, ... */
  pts: Float64Array;
  closed: boolean;
  box: Box;
}

/** Tramo consecutivo de segmentos de un contorno; unidad de descarte y de `Path2D`. */
export interface Chunk {
  box: Box;
  poly: number;
  /** Índices de punto (inclusive). */
  from: number;
  to: number;
  /** Segmento de cierre `último -> primero` del contorno (trozo propio). */
  closing: boolean;
  /** El trozo cubre todo el contorno y debe cerrarse con `closePath`. */
  close: boolean;
  /** El contorno al que pertenece es cerrado (para relleno y pruebas de punto dentro). */
  closed: boolean;
  /** Se crea al dibujar por primera vez. */
  path: Path2D | null;
}

export interface ObjectGeom {
  polys: PolyGeom[];
  chunks: Chunk[];
  box: Box | null;
  segments: number;
  image: boolean;
  /** Camino completo para rellenar; se crea al primer uso. */
  fill: Path2D | null;
}

const cache = new WeakMap<DesignObject, ObjectGeom>();

/**
 * Geometría en coordenadas de documento del objeto, calculada una vez. Los objetos del store son
 * inmutables (cada cambio crea uno nuevo), así que la entrada se invalida sola cuando cambia.
 */
export function getGeom(obj: DesignObject): ObjectGeom {
  let g = cache.get(obj);
  if (!g) {
    g = buildGeom(obj);
    cache.set(obj, g);
  }
  return g;
}

/** Para pruebas y mediciones. */
export function buildGeom(obj: DesignObject): ObjectGeom {
  const [a, b, c, d, e, f] = obj.transform;
  const polys: PolyGeom[] = [];
  const image = obj.kind.type === 'image';
  const closedFlags: boolean[] = [];

  let flat: Float64Array[] = [];
  if (obj.kind.type === 'vector') {
    for (const p of obj.kind.paths) {
      if (p.points.length === 0) continue;
      const arr = new Float64Array(p.points.length * 2);
      for (let i = 0; i < p.points.length; i++) {
        const px = p.points[i][0];
        const py = p.points[i][1];
        arr[i * 2] = a * px + c * py + e;
        arr[i * 2 + 1] = b * px + d * py + f;
      }
      flat.push(arr);
      closedFlags.push(p.closed);
    }
  } else {
    const [w, h] = obj.kind.size_mm;
    const local = [0, 0, w, 0, w, h, 0, h];
    const arr = new Float64Array(8);
    for (let i = 0; i < 4; i++) {
      const px = local[i * 2];
      const py = local[i * 2 + 1];
      arr[i * 2] = a * px + c * py + e;
      arr[i * 2 + 1] = b * px + d * py + f;
    }
    flat = [arr];
    closedFlags.push(true);
  }

  const chunks: Chunk[] = [];
  let box: Box | null = null;
  let segments = 0;
  flat.forEach((pts, pi) => {
    const closed = closedFlags[pi];
    const n = pts.length / 2;
    const pbox = boxOf(pts, 0, n - 1);
    polys.push({ pts, closed, box: pbox });
    box = box ? unionBox(box, pbox) : pbox;
    if (n < 2) return;
    const closes = closed && n > 2;
    segments += n - 1 + (closes ? 1 : 0);
    if (closes && n - 1 <= CHUNK_SEGMENTS) {
      chunks.push({ box: pbox, poly: pi, from: 0, to: n - 1, closing: false, close: true, closed, path: null });
      return;
    }
    for (let i0 = 0; i0 < n - 1; ) {
      const i1 = Math.min(i0 + CHUNK_SEGMENTS, n - 1);
      chunks.push({ box: boxOf(pts, i0, i1), poly: pi, from: i0, to: i1, closing: false, close: false, closed, path: null });
      i0 = i1;
    }
    if (closes) {
      const closing = [pts[(n - 1) * 2], pts[(n - 1) * 2 + 1], pts[0], pts[1]];
      chunks.push({
        box: [Math.min(closing[0], closing[2]), Math.min(closing[1], closing[3]), Math.max(closing[0], closing[2]), Math.max(closing[1], closing[3])],
        poly: pi, from: n - 1, to: 0, closing: true, close: false, closed, path: null,
      });
    }
  });
  return { polys, chunks, box, segments, image, fill: null };
}

function boxOf(pts: Float64Array, i0: number, i1: number): Box {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (let i = i0; i <= i1; i++) {
    const x = pts[i * 2];
    const y = pts[i * 2 + 1];
    if (x < minX) minX = x;
    if (x > maxX) maxX = x;
    if (y < minY) minY = y;
    if (y > maxY) maxY = y;
  }
  return [minX, minY, maxX, maxY];
}

export function unionBox(a: Box, b: Box): Box {
  return [Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.max(a[2], b[2]), Math.max(a[3], b[3])];
}

export function boxesIntersect(a: Box, b: Box): boolean {
  return a[0] <= b[2] && a[2] >= b[0] && a[1] <= b[3] && a[3] >= b[1];
}

export function boxContains(outer: Box, inner: Box): boolean {
  return inner[0] >= outer[0] && inner[2] <= outer[2] && inner[1] >= outer[1] && inner[3] <= outer[3];
}

/** `Path2D` de un trozo (se crea una vez por trozo). */
export function chunkPath(g: ObjectGeom, ch: Chunk): Path2D {
  if (ch.path) return ch.path;
  const pts = g.polys[ch.poly].pts;
  const p = new Path2D();
  if (ch.closing) {
    p.moveTo(pts[ch.from * 2], pts[ch.from * 2 + 1]);
    p.lineTo(pts[0], pts[1]);
  } else {
    p.moveTo(pts[ch.from * 2], pts[ch.from * 2 + 1]);
    for (let i = ch.from + 1; i <= ch.to; i++) p.lineTo(pts[i * 2], pts[i * 2 + 1]);
    if (ch.close) p.closePath();
  }
  ch.path = p;
  return p;
}

/** `Path2D` con todos los contornos, para rellenar con regla par-impar. */
export function fillPath(g: ObjectGeom): Path2D {
  if (g.fill) return g.fill;
  const p = new Path2D();
  for (const poly of g.polys) {
    const n = poly.pts.length / 2;
    if (n < 2) continue;
    p.moveTo(poly.pts[0], poly.pts[1]);
    for (let i = 1; i < n; i++) p.lineTo(poly.pts[i * 2], poly.pts[i * 2 + 1]);
    if (poly.closed) p.closePath();
  }
  g.fill = p;
  return p;
}

/** Recorre los segmentos de un trozo. El callback devuelve `true` para cortar el recorrido. */
export function eachSegment(g: ObjectGeom, ch: Chunk, cb: (x1: number, y1: number, x2: number, y2: number) => boolean | void): boolean {
  const pts = g.polys[ch.poly].pts;
  if (ch.closing) return cb(pts[ch.from * 2], pts[ch.from * 2 + 1], pts[0], pts[1]) === true;
  for (let i = ch.from; i < ch.to; i++) {
    if (cb(pts[i * 2], pts[i * 2 + 1], pts[i * 2 + 2], pts[i * 2 + 3]) === true) return true;
  }
  if (ch.close) return cb(pts[ch.to * 2], pts[ch.to * 2 + 1], pts[0], pts[1]) === true;
  return false;
}

/** Región visible en mm (con margen en px) para un lienzo de `w` x `h` px. */
export function visibleBox(view: { zoom: number; panX: number; panY: number }, w: number, h: number, marginPx = 0): Box {
  const { zoom, panX, panY } = view;
  return [(-marginPx - panX) / zoom, (panY - h - marginPx) / zoom, (w + marginPx - panX) / zoom, (panY + marginPx) / zoom];
}
