import type { BezPath } from '@/lib/bezpath';
import type { Point } from '../types';

/**
 * Revertir un redondeo en el motor simulado: el mismo algoritmo que `tlaku_geom::unfillet_path`.
 * Devuelve a punta una esquina redondeada (arco tangente a dos rectas) o mordida (arco centrado en
 * la esquina); el chaflán no se revierte porque no se distingue de un lado cualquiera.
 */

type Edge = { kind: 'line' } | { kind: 'cubic'; c1: Point; c2: Point };
interface Sub {
  nodes: Point[];
  /** `edges[i]` une `nodes[i]` con `nodes[(i + 1) % n]`. */
  edges: Edge[];
  closed: boolean;
}
interface Run {
  start: number;
  len: number;
}

const MIN_TURN_SIN = 0.017;
const ARC_TOL = 1e-3;
const SAMPLES = 8;
const MAX_RUN = 4;

const dist = (a: Point, b: Point) => Math.hypot(a[0] - b[0], a[1] - b[1]);

function unit(a: Point, b: Point): Point {
  const l = dist(a, b);
  return [(b[0] - a[0]) / l, (b[1] - a[1]) / l];
}

function cubicAt(a: Point, c1: Point, c2: Point, b: Point, t: number): Point {
  const u = 1 - t;
  const w = [u * u * u, 3 * u * u * t, 3 * u * t * t, t * t * t];
  return [w[0]! * a[0] + w[1]! * c1[0] + w[2]! * c2[0] + w[3]! * b[0], w[0]! * a[1] + w[1]! * c1[1] + w[2]! * c2[1] + w[3]! * b[1]];
}

function parse(sub: BezPath): Sub | null {
  const nodes: Point[] = [];
  const edges: Edge[] = [];
  let closed = false;
  for (const s of sub) {
    if (s.type === 'move_to') {
      if (nodes.length > 0) return null;
      nodes.push(s.to);
    } else if (s.type === 'line_to') {
      const last = nodes[nodes.length - 1];
      if (!last) return null;
      if (dist(last, s.to) > 1e-9) {
        nodes.push(s.to);
        edges.push({ kind: 'line' });
      }
    } else if (s.type === 'quad_to') {
      const p0 = nodes[nodes.length - 1];
      if (!p0) return null;
      const k = 2 / 3;
      nodes.push(s.to);
      edges.push({
        kind: 'cubic',
        c1: [p0[0] + (s.ctrl[0] - p0[0]) * k, p0[1] + (s.ctrl[1] - p0[1]) * k],
        c2: [s.to[0] + (s.ctrl[0] - s.to[0]) * k, s.to[1] + (s.ctrl[1] - s.to[1]) * k],
      });
    } else if (s.type === 'cubic_to') {
      nodes.push(s.to);
      edges.push({ kind: 'cubic', c1: s.ctrl1, c2: s.ctrl2 });
    } else closed = true;
  }
  if (nodes.length < 2) return null;
  if (nodes.length >= 4 && dist(nodes[0]!, nodes[nodes.length - 1]!) < 1e-9) {
    nodes.pop();
    closed = true;
  } else if (closed) edges.push({ kind: 'line' });
  if (closed && nodes.length < 3) return null;
  return { nodes, edges, closed };
}

const isCubic = (sub: Sub, e: number) => sub.edges[e]!.kind === 'cubic';

function runsOf(sub: Sub): Run[] {
  const m = sub.edges.length;
  const out: Run[] = [];
  for (let e = 0; e < m; e++) {
    if (!isCubic(sub, e)) continue;
    let prev: number;
    if (sub.closed) prev = (e + m - 1) % m;
    else if (e === 0) continue;
    else prev = e - 1;
    if (isCubic(sub, prev)) continue;
    let len = 1;
    while (len <= MAX_RUN) {
      const next = e + len;
      const idx = sub.closed ? next % m : next;
      if (idx >= m || (sub.closed && len >= m)) {
        len = 0;
        break;
      }
      if (!isCubic(sub, idx)) break;
      len++;
    }
    if (len === 0 || len > MAX_RUN) continue;
    out.push({ start: e, len });
  }
  return out;
}

function cornerOf(sub: Sub, run: Run): { v: Point; samples: Point[] } | null {
  const n = sub.nodes.length;
  const a = run.start;
  const b = (a + run.len) % n;
  const p = sub.nodes[(a + n - 1) % n]!;
  const na = sub.nodes[a]!;
  const nb = sub.nodes[b]!;
  const nn = sub.nodes[(b + 1) % n]!;
  if (dist(p, na) < 1e-9 || dist(nb, nn) < 1e-9) return null;
  const d1 = unit(p, na);
  const d2 = unit(nb, nn);
  const cross = d1[0] * d2[1] - d1[1] * d2[0];
  if (Math.abs(cross) < MIN_TURN_SIN) return null;
  const rx = nb[0] - na[0];
  const ry = nb[1] - na[1];
  const s = (rx * d2[1] - ry * d2[0]) / cross;
  const t = (d1[0] * ry - d1[1] * rx) / cross;
  if (!(s > 1e-6 && t > 1e-6) || Math.abs(s - t) > ARC_TOL * Math.max(s, t) + 1e-6) return null;
  const v: Point = [na[0] + d1[0] * s, na[1] + d1[1] * s];

  const checks: Point[] = [na];
  const samples: Point[] = [v];
  for (let k = 0; k < run.len; k++) {
    const e = (a + k) % sub.edges.length;
    const edge = sub.edges[e]!;
    if (edge.kind !== 'cubic') return null;
    const from = sub.nodes[e]!;
    const to = sub.nodes[(e + 1) % n]!;
    checks.push(to, cubicAt(from, edge.c1, edge.c2, to, 0.5));
    for (let i = 0; i <= SAMPLES; i++) samples.push(cubicAt(from, edge.c1, edge.c2, to, i / SAMPLES));
  }
  const u = unit(v, na);
  const w = unit(v, nb);
  const th = Math.acos(Math.min(1, Math.max(-1, u[0] * w[0] + u[1] * w[1])));
  const onCircle = (c: Point, r: number) => checks.every((q) => Math.abs(dist(q, c) - r) <= ARC_TOL * r + 1e-6);
  const half = th / 2;
  const r = s * Math.tan(half);
  const bx = u[0] + w[0];
  const by = u[1] + w[1];
  const bl = Math.hypot(bx, by);
  const d = r / Math.sin(half);
  const center: Point = [v[0] + (bx / bl) * d, v[1] + (by / bl) * d];
  if (th > 1e-3 && th < Math.PI - 1e-3 && onCircle(center, r)) return { v, samples };
  if (onCircle(v, s)) return { v, samples };
  return null;
}

function restore(sub: Sub, run: Run, v: Point): BezPath | null {
  const n = sub.nodes.length;
  const m = sub.edges.length;
  const shift = sub.closed ? (run.start + n - 1) % n : 0;
  const node = (k: number) => sub.nodes[(k + shift) % n]!;
  const edge = (k: number) => sub.edges[(k + shift) % m]!;
  const a = sub.closed ? 1 : run.start;
  const after = a + run.len + 1;
  const nodes: Point[] = [];
  for (let k = 0; k < a; k++) nodes.push(node(k));
  nodes.push(v);
  for (let k = after; k < n; k++) nodes.push(node(k));
  const edges: Edge[] = [];
  for (let k = 0; k < a; k++) edges.push(edge(k));
  for (let k = after - 1; k < m; k++) edges.push(edge(k));
  if ((sub.closed && nodes.length < 3) || nodes.length < 2) return null;
  const out: BezPath = [{ type: 'move_to', to: nodes[0]! }];
  edges.forEach((e, k) => {
    const to = nodes[(k + 1) % nodes.length]!;
    const closing = sub.closed && k + 1 === edges.length;
    if (e.kind === 'line') {
      if (!closing) out.push({ type: 'line_to', to });
    } else out.push({ type: 'cubic_to', ctrl1: e.c1, ctrl2: e.c2, to });
  });
  if (sub.closed) out.push({ type: 'close' });
  return out;
}

/**
 * Devuelve a punta la esquina redondeada más cercana a `point` (a lo más `maxDist` mm), o `null` si no
 * hay ninguna que se pueda revertir de forma fiable. `subs` son los subtrayectos del objeto.
 */
export function unfilletSubs(subs: readonly BezPath[], point: Point, maxDist: number): { subs: BezPath[]; distance: number } | null {
  if (!(Number.isFinite(maxDist) && maxDist > 0 && Number.isFinite(point[0]) && Number.isFinite(point[1]))) return null;
  let best: { index: number; sub: Sub; run: Run; v: Point; d: number } | null = null;
  subs.forEach((raw, index) => {
    const sub = parse(raw);
    if (!sub) return;
    for (const run of runsOf(sub)) {
      const corner = cornerOf(sub, run);
      if (!corner) continue;
      const d = Math.min(...corner.samples.map((q) => dist(q, point)));
      if (d <= maxDist && (!best || d < best.d)) best = { index, sub, run, v: corner.v, d };
    }
  });
  const found = best as { index: number; sub: Sub; run: Run; v: Point; d: number } | null;
  if (!found) return null;
  const restored = restore(found.sub, found.run, found.v);
  if (!restored) return null;
  return { subs: subs.map((s, i) => (i === found.index ? restored : s)), distance: found.d };
}
