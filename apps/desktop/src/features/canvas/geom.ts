import type { Affine, BezPath, DesignObject, Point, TextObject } from '@/api/types';
import { cubicAt, cubicExtremePoints, FLATTEN_TOLERANCE_MM, quadToCubic } from '@/lib/bezpath';
import { objectLocalPaths } from '@/lib/geometry';

/** Caja [minX, minY, maxX, maxY] en mm. Plana y sin asignaciones: se usa en rutas calientes. */
export type Box = [number, number, number, number];

/** Puntos por trozo de trazo: trozos pequeños permiten descartar lo que queda fuera de la vista. */
export const CHUNK_SEGMENTS = 1024;

/** Operaciones de un contorno (el inicio se guarda aparte). */
export const OP_LINE = 1;
export const OP_QUAD = 2;
export const OP_CUBIC = 3;
export const OP_CLOSE = 4;

/**
 * Subtrayecto en coordenadas de documento (ya con el `transform` del objeto aplicado). Guarda las
 * dos formas: las curvas originales (`ops`/`co`, para dibujar con `quadraticCurveTo` y
 * `bezierCurveTo`, nítidas a cualquier zoom) y su versión aplanada (`pts`, para hit-test, cajas y
 * relleno).
 */
export interface PolyGeom {
  /** Aplanado con `FLATTEN_TOLERANCE_MM`: x0, y0, x1, y1, ... */
  pts: Float64Array;
  closed: boolean;
  /** Caja exacta (incluye los extremos de las curvas). */
  box: Box;
  ops: Uint8Array;
  /** Inicio (x, y) seguido de las coordenadas de cada operación. */
  co: Float64Array;
  /** Posición en `co` donde empieza cada operación (longitud `ops + 1`). */
  coAt: Uint32Array;
  /** Índice en `pts` del último punto que genera cada operación. */
  ptEnd: Uint32Array;
}

/** Tramo consecutivo de operaciones de un contorno; unidad de descarte y de `Path2D`. */
export interface Chunk {
  box: Box;
  poly: number;
  /** Índices de punto aplanado (inclusive). */
  from: number;
  to: number;
  /** Operaciones [o0, o1) del contorno que cubre el trozo. */
  o0: number;
  o1: number;
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
  /** Segmentos aplanados: mide cuánto cuesta (descarte y bitmap en diseños densos). */
  segments: number;
  image: boolean;
  /** Texto sin contorno generado: se dibuja un marcador con el texto del sistema. */
  placeholder: TextObject | null;
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

interface RawSub {
  ops: number[];
  co: number[];
}

/** Parte las trayectorias en subtrayectos (cada uno con al menos un trazo) y les aplica `m`. */
function parseSubs(paths: readonly BezPath[], m: Affine): RawSub[] {
  const [a, b, c, d, e, f] = m;
  const tx = ([x, y]: Point) => a * x + c * y + e;
  const ty = ([x, y]: Point) => b * x + d * y + f;
  const out: RawSub[] = [];
  for (const path of paths) {
    let cur = null as RawSub | null; // (el cast evita que TS lo estreche a `null`: lo asignan los cierres)
    let start = null as Point | null; // inicio (ya transformado) del último subtrayecto
    const flush = () => {
      if (cur && cur.ops.length > 0) out.push(cur);
      cur = null;
    };
    const begin = (x: number, y: number) => {
      cur = { ops: [], co: [x, y] };
      start = [x, y];
    };
    /** Garantiza un subtrayecto abierto; `false` si el punto dado se tomó como su inicio. */
    const ensure = (to: Point): RawSub | null => {
      if (cur) return cur;
      if (start) {
        begin(start[0], start[1]); // tras `close`, lo que siga arranca donde empezó el anterior
        return cur;
      }
      begin(tx(to), ty(to));
      return null;
    };
    for (const s of path) {
      switch (s.type) {
        case 'move_to':
          flush();
          begin(tx(s.to), ty(s.to));
          break;
        case 'line_to': {
          const sub = ensure(s.to);
          if (!sub) break;
          sub.ops.push(OP_LINE);
          sub.co.push(tx(s.to), ty(s.to));
          break;
        }
        case 'quad_to': {
          const sub = ensure(s.to);
          if (!sub) break;
          sub.ops.push(OP_QUAD);
          sub.co.push(tx(s.ctrl), ty(s.ctrl), tx(s.to), ty(s.to));
          break;
        }
        case 'cubic_to': {
          const sub = ensure(s.to);
          if (!sub) break;
          sub.ops.push(OP_CUBIC);
          sub.co.push(tx(s.ctrl1), ty(s.ctrl1), tx(s.ctrl2), ty(s.ctrl2), tx(s.to), ty(s.to));
          break;
        }
        case 'close':
          if (cur && cur.ops.length > 0) {
            cur.ops.push(OP_CLOSE);
            flush();
          }
          break;
      }
    }
    flush();
  }
  return out;
}

const segCount = (maxSecondDiff: number, factor: number): number => {
  const n = Math.ceil(Math.sqrt((factor * maxSecondDiff) / FLATTEN_TOLERANCE_MM));
  return Number.isFinite(n) ? Math.min(1000, Math.max(1, n)) : 1;
};

/** Aplana un subtrayecto (mismo algoritmo que `BezPath::flatten` del motor) y calcula su caja exacta. */
function buildPoly(raw: RawSub): PolyGeom {
  const { co } = raw;
  const nOps = raw.ops.length;
  const ops = Uint8Array.from(raw.ops);
  const coAt = new Uint32Array(nOps + 1);
  const ptEnd = new Uint32Array(nOps);
  const pts: number[] = [co[0], co[1]];
  let minX = co[0];
  let maxX = co[0];
  let minY = co[1];
  let maxY = co[1];
  const grow = (x: number, y: number) => {
    if (x < minX) minX = x;
    if (x > maxX) maxX = x;
    if (y < minY) minY = y;
    if (y > maxY) maxY = y;
  };
  let c = 2;
  let px = co[0];
  let py = co[1];
  for (let k = 0; k < nOps; k++) {
    coAt[k] = c;
    switch (ops[k]) {
      case OP_LINE:
        px = co[c];
        py = co[c + 1];
        pts.push(px, py);
        grow(px, py);
        c += 2;
        break;
      case OP_QUAD: {
        const [cx, cy, x, y] = [co[c], co[c + 1], co[c + 2], co[c + 3]];
        const n = segCount(Math.hypot(px - 2 * cx + x, py - 2 * cy + y), 0.25);
        for (let i = 1; i <= n; i++) {
          const t = i / n;
          const u = 1 - t;
          const qx = u * u * px + 2 * u * t * cx + t * t * x;
          const qy = u * u * py + 2 * u * t * cy + t * t * y;
          pts.push(qx, qy);
          grow(qx, qy);
        }
        const [c1, c2] = quadToCubic([px, py], [cx, cy], [x, y]);
        for (const e of cubicExtremePoints([px, py], c1, c2, [x, y])) grow(e[0], e[1]);
        px = x;
        py = y;
        c += 4;
        break;
      }
      case OP_CUBIC: {
        const c1: Point = [co[c], co[c + 1]];
        const c2: Point = [co[c + 2], co[c + 3]];
        const to: Point = [co[c + 4], co[c + 5]];
        const d = Math.max(
          Math.hypot(px - 2 * c1[0] + c2[0], py - 2 * c1[1] + c2[1]),
          Math.hypot(c1[0] - 2 * c2[0] + to[0], c1[1] - 2 * c2[1] + to[1]),
        );
        const n = segCount(d, 0.75);
        const p0: Point = [px, py];
        for (let i = 1; i <= n; i++) {
          const q = cubicAt(p0, c1, c2, to, i / n);
          pts.push(q[0], q[1]);
          grow(q[0], q[1]);
        }
        for (const e of cubicExtremePoints(p0, c1, c2, to)) grow(e[0], e[1]);
        px = to[0];
        py = to[1];
        c += 6;
        break;
      }
      default: // OP_CLOSE
        break;
    }
    ptEnd[k] = pts.length / 2 - 1;
  }
  coAt[nOps] = c;
  const flat = Float64Array.from(pts);
  const n = flat.length / 2;
  const hasClose = nOps > 0 && ops[nOps - 1] === OP_CLOSE;
  const implicit = n > 2 && Math.hypot(flat[0] - flat[(n - 1) * 2], flat[1] - flat[(n - 1) * 2 + 1]) < 1e-9;
  return {
    pts: flat,
    closed: hasClose || implicit,
    box: [minX, minY, maxX, maxY],
    ops,
    co: Float64Array.from(co),
    coAt,
    ptEnd,
  };
}

/** Para pruebas y mediciones. */
export function buildGeom(obj: DesignObject): ObjectGeom {
  const kind = obj.kind;
  const image = kind.type === 'image';
  const placeholder = kind.type === 'text' && kind.outline.length === 0 ? kind : null;
  const polys = parseSubs(objectLocalPaths(obj), obj.transform).map(buildPoly);

  const chunks: Chunk[] = [];
  let box: Box | null = null;
  let segments = 0;
  polys.forEach((poly, pi) => {
    box = box ? unionBox(box, poly.box) : poly.box;
    segments += chunkPoly(poly, pi, chunks);
  });
  return { polys, chunks, box, segments, image, placeholder, fill: null };
}

/** Parte un contorno en trozos descartables y devuelve cuántos segmentos aplanados tiene. */
function chunkPoly(poly: PolyGeom, pi: number, chunks: Chunk[]): number {
  const { pts, ops, ptEnd, closed } = poly;
  const n = pts.length / 2;
  const nOps = ops.length;
  const hasClose = ops[nOps - 1] === OP_CLOSE;
  const drawOps = hasClose ? nOps - 1 : nOps;
  const closes = hasClose && n > 2;
  const segs = n - 1 + (closes ? 1 : 0);
  if (closes && n - 1 <= CHUNK_SEGMENTS) {
    chunks.push({ box: poly.box, poly: pi, from: 0, to: n - 1, o0: 0, o1: drawOps, closing: false, close: true, closed, path: null });
    return segs;
  }
  for (let o0 = 0; o0 < drawOps; ) {
    const from = o0 === 0 ? 0 : ptEnd[o0 - 1];
    let o1 = o0 + 1;
    while (o1 < drawOps && ptEnd[o1 - 1] - from < CHUNK_SEGMENTS) o1++;
    const to = ptEnd[o1 - 1];
    chunks.push({ box: boxOf(pts, from, to), poly: pi, from, to, o0, o1, closing: false, close: false, closed, path: null });
    o0 = o1;
  }
  if (closes) {
    const closing = [pts[(n - 1) * 2], pts[(n - 1) * 2 + 1], pts[0], pts[1]];
    chunks.push({
      box: [Math.min(closing[0], closing[2]), Math.min(closing[1], closing[3]), Math.max(closing[0], closing[2]), Math.max(closing[1], closing[3])],
      poly: pi, from: n - 1, to: 0, o0: 0, o1: 0, closing: true, close: false, closed, path: null,
    });
  }
  return segs;
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

/** Añade las operaciones [o0, o1) del contorno a `p` (curvas reales, no segmentos). */
function appendOps(p: Path2D, poly: PolyGeom, o0: number, o1: number): void {
  const { ops, co, coAt } = poly;
  for (let k = o0; k < o1; k++) {
    const c = coAt[k];
    switch (ops[k]) {
      case OP_LINE:
        p.lineTo(co[c], co[c + 1]);
        break;
      case OP_QUAD:
        p.quadraticCurveTo(co[c], co[c + 1], co[c + 2], co[c + 3]);
        break;
      case OP_CUBIC:
        p.bezierCurveTo(co[c], co[c + 1], co[c + 2], co[c + 3], co[c + 4], co[c + 5]);
        break;
      default:
        p.closePath();
    }
  }
}

/** `Path2D` de un trozo con las curvas originales (se crea una vez por trozo). */
export function chunkPath(g: ObjectGeom, ch: Chunk, make: () => Path2D = () => new Path2D()): Path2D {
  if (ch.path) return ch.path;
  const poly = g.polys[ch.poly];
  const pts = poly.pts;
  const p = make();
  if (ch.closing) {
    p.moveTo(pts[ch.from * 2], pts[ch.from * 2 + 1]);
    p.lineTo(pts[0], pts[1]);
  } else {
    p.moveTo(pts[ch.from * 2], pts[ch.from * 2 + 1]);
    appendOps(p, poly, ch.o0, ch.o1);
    if (ch.close) p.closePath();
  }
  ch.path = p;
  return p;
}

/** `Path2D` con todos los contornos (con curvas), para rellenar con regla par-impar. */
export function fillPath(g: ObjectGeom, make: () => Path2D = () => new Path2D()): Path2D {
  if (g.fill) return g.fill;
  const p = make();
  for (const poly of g.polys) {
    p.moveTo(poly.co[0], poly.co[1]);
    appendOps(p, poly, 0, poly.ops.length);
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
