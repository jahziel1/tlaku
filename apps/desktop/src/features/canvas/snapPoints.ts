import type { DesignObject, Point } from '@/api/types';
import { cubicAt } from '@/lib/bezpath';
import { eachSegment, getGeom, OP_CLOSE, OP_CUBIC, OP_LINE, OP_QUAD, type Box, type ObjectGeom } from './geom';

/**
 * Puntos de ajuste magnético de los objetos: nodos, puntos medios de cada segmento, centros e
 * intersecciones entre trazos. Cada punto lleva su tipo para dibujar un indicador distinto.
 */
export type SnapPointKind = 'node' | 'mid' | 'center' | 'inter';

export interface SnapPoint {
  p: Point;
  kind: SnapPointKind;
}

/** Un objeto con más segmentos que esto no aporta puntos (mantiene ágil el arrastre en diseños densos). */
const MAX_SEGMENTS_PER_OBJECT = 20_000;
const MAX_POINTS = 60_000;
/** Con más segmentos que esto cerca del cursor no se calculan intersecciones. */
const MAX_NEAR_SEGMENTS = 600;

/** Prioridad al elegir entre dos puntos igual de cerca: más específico primero. */
const RANK: Record<SnapPointKind, number> = { inter: 0, node: 1, mid: 2, center: 3 };

/** Nodos, puntos medios y centro de un objeto (coordenadas de documento). */
export function objectSnapPoints(obj: DesignObject, out: SnapPoint[] = []): SnapPoint[] {
  const g = getGeom(obj);
  if (!g.box || g.segments > MAX_SEGMENTS_PER_OBJECT) return out;
  for (const poly of g.polys) {
    const { co, ops, coAt } = poly;
    let px = co[0]!;
    let py = co[1]!;
    const sx = px;
    const sy = py;
    out.push({ p: [px, py], kind: 'node' });
    // Un contorno que vuelve a su inicio (la elipse) no repite el nodo.
    const isStart = (x: number, y: number) => Math.hypot(x - sx, y - sy) < 1e-9;
    for (let k = 0; k < ops.length; k++) {
      const c = coAt[k]!;
      switch (ops[k]) {
        case OP_LINE: {
          const x = co[c]!;
          const y = co[c + 1]!;
          out.push({ p: [(px + x) / 2, (py + y) / 2], kind: 'mid' });
          if (!isStart(x, y)) out.push({ p: [x, y], kind: 'node' });
          px = x;
          py = y;
          break;
        }
        case OP_QUAD: {
          const [qx, qy, x, y] = [co[c]!, co[c + 1]!, co[c + 2]!, co[c + 3]!];
          out.push({ p: [0.25 * px + 0.5 * qx + 0.25 * x, 0.25 * py + 0.5 * qy + 0.25 * y], kind: 'mid' });
          if (!isStart(x, y)) out.push({ p: [x, y], kind: 'node' });
          px = x;
          py = y;
          break;
        }
        case OP_CUBIC: {
          const to: Point = [co[c + 4]!, co[c + 5]!];
          out.push({ p: cubicAt([px, py], [co[c]!, co[c + 1]!], [co[c + 2]!, co[c + 3]!], to, 0.5), kind: 'mid' });
          if (!isStart(to[0], to[1])) out.push({ p: to, kind: 'node' });
          px = to[0];
          py = to[1];
          break;
        }
        case OP_CLOSE:
          // El cierre es un segmento recto hasta el inicio (si no cae ya sobre él).
          if (Math.hypot(px - sx, py - sy) > 1e-9) out.push({ p: [(px + sx) / 2, (py + sy) / 2], kind: 'mid' });
          break;
        default:
          break;
      }
    }
  }
  out.push({ p: [(g.box[0] + g.box[2]) / 2, (g.box[1] + g.box[3]) / 2], kind: 'center' });
  return out;
}

/** Puntos de todos los objetos visibles (menos los de `skip`) cuya caja toca `region`. */
export function collectSnapPoints(objects: readonly DesignObject[], skip: ReadonlySet<string>, region?: Box): SnapPoint[] {
  const out: SnapPoint[] = [];
  for (const obj of objects) {
    if (obj.hidden || skip.has(obj.id)) continue;
    const box = getGeom(obj).box;
    if (!box) continue;
    if (region && (box[2] < region[0] || box[0] > region[2] || box[3] < region[1] || box[1] > region[3])) continue;
    objectSnapPoints(obj, out);
    if (out.length > MAX_POINTS) break;
  }
  return out;
}

/** El punto más cercano a `p` dentro de `tol` (mm); a igual distancia gana el tipo más específico. */
export function nearestSnapPoint(points: readonly SnapPoint[], p: Point, tol: number): SnapPoint | null {
  let best: SnapPoint | null = null;
  let bestD = tol + 1e-9;
  for (const sp of points) {
    const d = Math.hypot(sp.p[0] - p[0], sp.p[1] - p[1]);
    if (d < bestD - 1e-9 || (d <= bestD + 1e-9 && best && RANK[sp.kind] < RANK[best.kind])) {
      best = sp;
      bestD = d;
    }
  }
  return best;
}

// ---- intersecciones ---------------------------------------------------------------------------------

type Seg = [number, number, number, number];

/** Cruce propio de dos segmentos (sin contar que compartan un extremo), o `null`. */
export function segmentIntersection(a: Seg, b: Seg): Point | null {
  const [x1, y1, x2, y2] = a;
  const [x3, y3, x4, y4] = b;
  const d = (x2 - x1) * (y4 - y3) - (y2 - y1) * (x4 - x3);
  if (Math.abs(d) < 1e-12) return null; // paralelos
  const t = ((x3 - x1) * (y4 - y3) - (y3 - y1) * (x4 - x3)) / d;
  const u = ((x3 - x1) * (y2 - y1) - (y3 - y1) * (x2 - x1)) / d;
  const eps = 1e-9;
  if (t <= eps || t >= 1 - eps || u <= eps || u >= 1 - eps) return null;
  return [x1 + t * (x2 - x1), y1 + t * (y2 - y1)];
}

function segmentsNear(geoms: readonly ObjectGeom[], p: Point, r: number): Seg[] {
  const window: Box = [p[0] - r, p[1] - r, p[0] + r, p[1] + r];
  const out: Seg[] = [];
  for (const g of geoms) {
    if (!g.box || g.box[2] < window[0] || g.box[0] > window[2] || g.box[3] < window[1] || g.box[1] > window[3]) continue;
    for (const ch of g.chunks) {
      const b = ch.box;
      if (b[2] < window[0] || b[0] > window[2] || b[3] < window[1] || b[1] > window[3]) continue;
      eachSegment(g, ch, (x1, y1, x2, y2) => {
        if (Math.max(x1, x2) < window[0] || Math.min(x1, x2) > window[2] || Math.max(y1, y2) < window[1] || Math.min(y1, y2) > window[3]) return;
        out.push([x1, y1, x2, y2]);
        return out.length > MAX_NEAR_SEGMENTS;
      });
      if (out.length > MAX_NEAR_SEGMENTS) return out;
    }
  }
  return out;
}

/** Intersecciones entre trazos a menos de `r` mm de `p` (se calculan solo cerca del cursor). */
export function intersectionsNear(geoms: readonly ObjectGeom[], p: Point, r: number): SnapPoint[] {
  const segs = segmentsNear(geoms, p, r * 3);
  if (segs.length < 2 || segs.length > MAX_NEAR_SEGMENTS) return [];
  const out: SnapPoint[] = [];
  for (let i = 0; i < segs.length; i++) {
    for (let j = i + 1; j < segs.length; j++) {
      const x = segmentIntersection(segs[i]!, segs[j]!);
      if (x && Math.hypot(x[0] - p[0], x[1] - p[1]) <= r) out.push({ p: x, kind: 'inter' });
    }
  }
  return out;
}
