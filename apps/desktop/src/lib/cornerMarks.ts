import type { CornerMark } from '@/bindings/CornerMark';
import type { BezPath } from './bezpath';

/**
 * Esquinas guardadas por la herramienta de redondear y achaflanar (D60, `VectorObject.corners`).
 * Cada marca guarda la punta original y los dos nodos entre los que quedó el arco o el chaflán.
 * Solo vale mientras la geometría siga igual: aquí se descartan las que el usuario dejó atrás al
 * editar nodos de esa zona. La misma regla vive en el motor (`edit/shape_ops/corner_marks.rs`),
 * que es quien las usa para revertir.
 */

type P = readonly [number, number];
type Edge = 'line' | 'cubic';
interface Ring {
  nodes: P[];
  edges: Edge[];
  closed: boolean;
}

const NODE_TOL = 1e-4;
const LINE_TOL = 1e-3;

const dist = (a: P, b: P) => Math.hypot(a[0] - b[0], a[1] - b[1]);

/** Parte un trayecto en subtrayectos y los reduce a nodos y aristas (recta o cúbica). */
function rings(path: BezPath): Ring[] {
  const out: Ring[] = [];
  let cur: Ring | null = null;
  const finish = () => {
    if (!cur) return;
    const r = cur;
    cur = null;
    if (r.nodes.length < 2) return;
    if (r.nodes.length >= 4 && dist(r.nodes[0]!, r.nodes[r.nodes.length - 1]!) < 1e-9) {
      r.nodes.pop();
      r.closed = true;
    } else if (r.closed) {
      r.edges.push('line');
    }
    if (r.closed && r.nodes.length < 3) return;
    out.push(r);
  };
  for (const seg of path) {
    if (seg.type === 'move_to') {
      finish();
      cur = { nodes: [seg.to], edges: [], closed: false };
    } else if (!cur) {
      continue;
    } else if (seg.type === 'close') {
      cur.closed = true;
      finish();
    } else {
      if (seg.type === 'line_to' && dist(cur.nodes[cur.nodes.length - 1]!, seg.to) <= 1e-9) continue;
      cur.nodes.push(seg.to);
      cur.edges.push(seg.type === 'line_to' ? 'line' : 'cubic');
    }
  }
  finish();
  return out;
}

const nextOf = (r: Ring, i: number): number | null => (i + 1 < r.nodes.length ? i + 1 : r.closed ? 0 : null);
const prevOf = (r: Ring, i: number): number | null => (i > 0 ? i - 1 : r.closed ? r.nodes.length - 1 : null);

function distToLine(p: P, a: P, b: P): number {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const l = Math.hypot(dx, dy);
  if (l < 1e-12) return dist(p, a);
  return Math.abs((p[0] - a[0]) * dy - (p[1] - a[1]) * dx) / l;
}

/** La punta está en la prolongación de `from -> to`, más allá de `to`. */
function ahead(from: P, to: P, v: P): boolean {
  return distToLine(v, from, to) <= LINE_TOL && (to[0] - from[0]) * (v[0] - to[0]) + (to[1] - from[1]) * (v[1] - to[1]) > 0;
}

function hasRun(r: Ring, a: P, b: P, v: P): boolean {
  const ia = r.nodes.findIndex((q) => dist(q, a) <= NODE_TOL);
  const ib = r.nodes.findIndex((q) => dist(q, b) <= NODE_TOL);
  if (ia < 0 || ib < 0 || ia === ib) return false;
  for (const [first, last] of [[ia, ib], [ib, ia]] as const) {
    let i = first;
    let len = 0;
    let lines = 0;
    let cubics = 0;
    let reached = false;
    for (;;) {
      if (i === last && len > 0) {
        reached = true;
        break;
      }
      const e = r.edges[i];
      if (len > r.nodes.length || e === undefined) break;
      if (e === 'line') lines++;
      else cubics++;
      len++;
      const j = nextOf(r, i);
      if (j === null) break;
      i = j;
    }
    if (!reached || !((lines === 1 && cubics === 0) || (lines === 0 && cubics > 0))) continue;
    const p = prevOf(r, first);
    const q = nextOf(r, last);
    if (p === null || q === null) continue;
    if (r.edges[p] === 'line' && r.edges[last] === 'line' && ahead(r.nodes[p]!, r.nodes[first]!, v) && ahead(r.nodes[q]!, r.nodes[last]!, v)) return true;
  }
  return false;
}

/** ¿Sigue valiendo la marca en estos trazos? */
export function cornerStillValid(paths: readonly BezPath[], mark: CornerMark): boolean {
  return paths.some((p) => rings(p).some((r) => hasRun(r, mark.from, mark.to, mark.vertex)));
}

/**
 * Las marcas que siguen valiendo tras editar los nodos. Devuelve `undefined` si no queda ninguna
 * (el campo se omite del objeto) y la misma lista si no cambió nada.
 */
export function pruneCorners(corners: readonly CornerMark[] | undefined, paths: readonly BezPath[]): CornerMark[] | undefined {
  if (!corners || corners.length === 0) return undefined;
  const kept = corners.filter((c) => cornerStillValid(paths, c));
  if (kept.length === 0) return undefined;
  return kept.length === corners.length ? (corners as CornerMark[]) : kept;
}
