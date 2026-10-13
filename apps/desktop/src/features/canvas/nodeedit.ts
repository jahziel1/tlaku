import type { Affine, BezPath, PathSeg, Point } from '@/api/types';
import { cubicAt, lerpPoint, quadToCubic, splitCubic } from '@/lib/bezpath';

/**
 * Modelo de edición de nodos: lógica pura, sin DOM ni store.
 *
 * Una trayectoria se ve como una lista de nodos con sus manejadores (puntos de control absolutos,
 * en coordenadas LOCALES del objeto). Un segmento entre dos nodos es una curva si alguno de los
 * dos manejadores que lo tocan existe; si no, es una recta. Todas las operaciones devuelven un
 * modelo nuevo (no modifican el recibido) y los nodos se nombran por su índice plano: el k-ésimo
 * nodo contando todos los subtrayectos en orden.
 */

export type NodeType = 'corner' | 'smooth' | 'symmetric';

export interface EditNode {
  p: Point;
  /** Manejador del segmento que llega al nodo (null = sin manejador). */
  inH: Point | null;
  /** Manejador del segmento que sale del nodo. */
  outH: Point | null;
}

export interface EditSub {
  nodes: EditNode[];
  /** Cerrado: el último nodo se une al primero con un segmento más. */
  closed: boolean;
  /** Trayectoria (`BezPath`) de origen; se conserva al reconstruir. */
  group: number;
}

export interface EditModel {
  subs: EditSub[];
}

export interface EditResult {
  model: EditModel;
  /** Nodos que deben quedar seleccionados (índices planos del modelo nuevo). */
  sel: number[];
}

/** Dos puntos más cerca que esto (mm) se consideran el mismo. */
export const JOIN_TOL = 1e-6;
const ANGLE_TOL = 1e-4;
const LEN_TOL = 1e-6;

// ---- vectores ---------------------------------------------------------------------------------------

const sub = (a: Point, b: Point): Point => [a[0] - b[0], a[1] - b[1]];
const add = (a: Point, b: Point): Point => [a[0] + b[0], a[1] + b[1]];
const mul = (a: Point, k: number): Point => [a[0] * k, a[1] * k];
const len = (a: Point): number => Math.hypot(a[0], a[1]);
const dist = (a: Point, b: Point): number => Math.hypot(a[0] - b[0], a[1] - b[1]);
const near = (a: Point, b: Point, tol = JOIN_TOL): boolean => dist(a, b) <= tol;
const unit = (a: Point, fallback: Point = [1, 0]): Point => {
  const l = len(a);
  return l < 1e-12 ? fallback : [a[0] / l, a[1] / l];
};

export function invertAffine(m: Affine): Affine | null {
  const [a, b, c, d, e, f] = m;
  const det = a * d - b * c;
  if (!Number.isFinite(det) || Math.abs(det) < 1e-12) return null;
  return [d / det, -b / det, -c / det, a / det, (c * f - d * e) / det, (b * e - a * f) / det];
}

export const applyPoint = (m: Affine, [x, y]: Point): Point => [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]];
/** Solo la parte lineal: sirve para vectores (desplazamientos). */
export const applyVector = (m: Affine, [x, y]: Point): Point => [m[0] * x + m[2] * y, m[1] * x + m[3] * y];

// ---- lectura y escritura de trayectorias --------------------------------------------------------------

const emptyNode = (p: Point): EditNode => ({ p, inH: null, outH: null });
const handleOrNull = (h: Point, p: Point): Point | null => (near(h, p, 1e-9) ? null : h);

/**
 * Lee las trayectorias como modelo. Las cuadráticas se elevan a cúbicas (misma forma). Solo
 * `close` cierra un subtrayecto en el modelo.
 */
export function parseModel(paths: readonly BezPath[]): EditModel {
  const subs: EditSub[] = [];
  paths.forEach((path, group) => {
    let cur: EditSub | null = null;
    let start: Point | null = null;
    const flush = () => {
      if (cur) finishSub(cur, subs);
      cur = null;
    };
    const ensure = (to: Point): EditSub => {
      if (cur) return cur;
      // Tras `close`, lo que siga arranca donde empezó el subtrayecto anterior.
      cur = { nodes: [emptyNode(start ?? to)], closed: false, group };
      start = cur.nodes[0].p;
      return cur;
    };
    for (const s of path) {
      switch (s.type) {
        case 'move_to':
          flush();
          start = s.to;
          cur = { nodes: [emptyNode(s.to)], closed: false, group };
          break;
        case 'line_to': {
          const c = ensure(s.to);
          c.nodes.push(emptyNode(s.to));
          break;
        }
        case 'quad_to': {
          const c = ensure(s.to);
          const prev = c.nodes[c.nodes.length - 1];
          const [h1, h2] = quadToCubic(prev.p, s.ctrl, s.to);
          prev.outH = handleOrNull(h1, prev.p);
          c.nodes.push({ p: s.to, inH: handleOrNull(h2, s.to), outH: null });
          break;
        }
        case 'cubic_to': {
          const c = ensure(s.to);
          const prev = c.nodes[c.nodes.length - 1];
          prev.outH = handleOrNull(s.ctrl1, prev.p);
          c.nodes.push({ p: s.to, inH: handleOrNull(s.ctrl2, s.to), outH: null });
          break;
        }
        case 'close':
          if (cur) {
            (cur as EditSub).closed = true;
            flush();
          }
          break;
      }
    }
    flush();
  });
  return { subs };
}

/**
 * Termina el subtrayecto leído: si está cerrado con `close` y su último nodo repite el primero, los funde;
 * descarta los de un solo nodo. Un abierto cuyos extremos coinciden sigue abierto (así "romper" deja dos
 * nodos que se pueden separar; "cerrar" los funde).
 */
function finishSub(c: EditSub, out: EditSub[]): void {
  const n = c.nodes.length;
  if (n < 2) return;
  if (c.closed && near(c.nodes[0].p, c.nodes[n - 1].p, 1e-9)) {
    c.nodes[0].inH = c.nodes[n - 1].inH;
    c.nodes.pop();
  }
  if (c.nodes.length < 2) return;
  out.push(c);
}

/** Segmento (a -> b) como trazo de la trayectoria. */
function segTo(a: EditNode, b: EditNode): PathSeg {
  if (a.outH || b.inH) return { type: 'cubic_to', ctrl1: a.outH ?? a.p, ctrl2: b.inH ?? b.p, to: b.p };
  return { type: 'line_to', to: b.p };
}

/** Reconstruye las trayectorias; cada subtrayecto vuelve a la trayectoria de la que salió. */
export function buildPaths(model: EditModel): BezPath[] {
  const byGroup = new Map<number, PathSeg[]>();
  for (const sub of model.subs) {
    if (sub.nodes.length === 0) continue;
    const segs = byGroup.get(sub.group) ?? [];
    byGroup.set(sub.group, segs);
    segs.push({ type: 'move_to', to: sub.nodes[0].p });
    for (let i = 1; i < sub.nodes.length; i++) segs.push(segTo(sub.nodes[i - 1], sub.nodes[i]));
    if (sub.closed) {
      const a = sub.nodes[sub.nodes.length - 1];
      const b = sub.nodes[0];
      if (a.outH || b.inH) segs.push(segTo(a, b));
      segs.push({ type: 'close' });
    }
  }
  return [...byGroup.keys()].sort((x, y) => x - y).map((g) => byGroup.get(g)!);
}

export function cloneModel(m: EditModel): EditModel {
  return { subs: m.subs.map((s) => ({ ...s, nodes: s.nodes.map((n) => ({ ...n })) })) };
}

// ---- índices ----------------------------------------------------------------------------------------------

export function nodeCount(m: EditModel): number {
  let n = 0;
  for (const s of m.subs) n += s.nodes.length;
  return n;
}

/** Subtrayecto y posición de un nodo por su índice plano. */
export function locate(m: EditModel, k: number): { s: number; i: number } | null {
  if (k < 0) return null;
  let rest = k;
  for (let s = 0; s < m.subs.length; s++) {
    const n = m.subs[s].nodes.length;
    if (rest < n) return { s, i: rest };
    rest -= n;
  }
  return null;
}

export function flatIndex(m: EditModel, s: number, i: number): number {
  let k = 0;
  for (let j = 0; j < s; j++) k += m.subs[j].nodes.length;
  return k + i;
}

/** Nodo por índice plano. */
export function nodeAt(m: EditModel, k: number): EditNode | null {
  const loc = locate(m, k);
  return loc ? m.subs[loc.s].nodes[loc.i] : null;
}

/** Índices planos (ordenados) de los nodos de `refs`. */
function indicesOf(m: EditModel, refs: ReadonlySet<EditNode>): number[] {
  const out: number[] = [];
  let k = 0;
  for (const s of m.subs) for (const n of s.nodes) {
    if (refs.has(n)) out.push(k);
    k++;
  }
  return out;
}

function refsOf(m: EditModel, sel: Iterable<number>): Set<EditNode> {
  const out = new Set<EditNode>();
  for (const k of sel) {
    const n = nodeAt(m, k);
    if (n) out.add(n);
  }
  return out;
}

/** Cuántos segmentos tiene el subtrayecto (el de cierre cuenta). */
export const segmentCount = (s: EditSub): number => (s.closed ? s.nodes.length : s.nodes.length - 1);

/** Segmento `i` de un subtrayecto: del nodo `i` al siguiente (el último de uno cerrado vuelve al primero). */
export function segmentEnds(s: EditSub, i: number): [EditNode, EditNode] {
  return [s.nodes[i], s.nodes[(i + 1) % s.nodes.length]];
}

export const isCurve = (a: EditNode, b: EditNode): boolean => a.outH !== null || b.inH !== null;

// ---- tipos de nodo -----------------------------------------------------------------------------------------------

/** Tipo que dice la geometría: esquina salvo que los dos manejadores estén alineados (y opuestos). */
export function inferNodeType(n: EditNode): NodeType {
  if (!n.inH || !n.outH) return 'corner';
  const a = sub(n.inH, n.p);
  const b = sub(n.outH, n.p);
  const la = len(a);
  const lb = len(b);
  if (la < LEN_TOL || lb < LEN_TOL) return 'corner';
  const cross = Math.abs(a[0] * b[1] - a[1] * b[0]) / (la * lb);
  const dot = a[0] * b[0] + a[1] * b[1];
  if (cross > ANGLE_TOL || dot >= 0) return 'corner';
  return Math.abs(la - lb) <= 1e-6 * Math.max(la, lb) + 1e-9 ? 'symmetric' : 'smooth';
}

/** Clave de posición de un nodo, para recordar que se pidió "esquina" aunque sus manejadores estén alineados. */
export function positionKey(p: Point): string {
  return `${Math.round(p[0] * 1e4)},${Math.round(p[1] * 1e4)}`;
}

/** Tipo efectivo: lo pedido explícitamente como esquina manda; si no, el que dice la geometría. */
export function nodeType(n: EditNode, corners?: ReadonlySet<string>): NodeType {
  if (corners?.has(positionKey(n.p))) return 'corner';
  return inferNodeType(n);
}

// ---- inserción ---------------------------------------------------------------------------------------------------

/** Divide el segmento `i` de `m` (se modifica) y devuelve el nodo nuevo. */
function insertInPlace(m: EditModel, s: number, i: number, t: number): EditNode | null {
  const sb = m.subs[s];
  if (!sb || i < 0 || i >= segmentCount(sb)) return null;
  const tt = Math.min(1, Math.max(0, t));
  const [a, b] = segmentEnds(sb, i);
  let fresh: EditNode;
  if (!isCurve(a, b)) {
    fresh = emptyNode(lerpPoint(a.p, b.p, tt));
  } else {
    const [l, r] = splitCubic(a.p, a.outH ?? a.p, b.inH ?? b.p, b.p, tt);
    a.outH = handleOrNull(l[1], a.p);
    b.inH = handleOrNull(r[2], b.p);
    fresh = { p: l[3], inH: handleOrNull(l[2], l[3]), outH: handleOrNull(r[1], r[3]) };
  }
  const at = i + 1 >= sb.nodes.length ? sb.nodes.length : i + 1; // el segmento de cierre añade al final
  sb.nodes.splice(at, 0, fresh);
  return fresh;
}

/** Divide el segmento `i` del subtrayecto `s` en `t` sin cambiar la forma. Devuelve el nodo nuevo, ya seleccionado. */
export function insertNode(model: EditModel, s: number, i: number, t: number): EditResult | null {
  const m = cloneModel(model);
  const fresh = insertInPlace(m, s, i, t);
  return fresh ? { model: m, sel: indicesOf(m, new Set([fresh])) } : null;
}

/** Segmentos cuyos dos extremos están seleccionados (con un solo nodo: sus dos segmentos si `lenient`). */
export function selectedSegments(model: EditModel, sel: readonly number[], lenient = false): { s: number; i: number }[] {
  const set = new Set(sel);
  const out: { s: number; i: number }[] = [];
  model.subs.forEach((sb, s) => {
    const base = flatIndex(model, s, 0);
    const n = sb.nodes.length;
    for (let i = 0; i < segmentCount(sb); i++) {
      if (set.has(base + i) && set.has(base + ((i + 1) % n))) out.push({ s, i });
    }
  });
  if (out.length === 0 && lenient && sel.length === 1) {
    const loc = locate(model, sel[0]);
    if (loc) {
      const sb = model.subs[loc.s];
      const n = sb.nodes.length;
      const prev = loc.i - 1 >= 0 ? loc.i - 1 : sb.closed ? n - 1 : -1;
      const next = loc.i < segmentCount(sb) ? loc.i : -1;
      if (prev >= 0) out.push({ s: loc.s, i: prev });
      if (next >= 0) out.push({ s: loc.s, i: next });
    }
  }
  return out;
}

/** Inserta un nodo en el punto medio de cada segmento entre nodos seleccionados. */
export function insertMidpoints(model: EditModel, sel: readonly number[]): EditResult | null {
  const segs = selectedSegments(model, sel);
  if (segs.length === 0) return null;
  const m = cloneModel(model);
  const made = new Set<EditNode>();
  // De atrás hacia delante: insertar en un segmento no cambia los índices de los anteriores.
  for (const { s, i } of [...segs].sort((a, b) => (b.s - a.s) || (b.i - a.i))) {
    const fresh = insertInPlace(m, s, i, 0.5);
    if (fresh) made.add(fresh);
  }
  return { model: m, sel: indicesOf(m, made) };
}

// ---- borrado ------------------------------------------------------------------------------------------------------

/**
 * Borra los nodos seleccionados y reconecta a sus vecinos (los manejadores que quedan se
 * conservan). Un trayecto que queda con menos de dos nodos desaparece.
 */
export function deleteNodes(model: EditModel, sel: readonly number[]): EditResult | null {
  const gone = refsOf(model, sel);
  if (gone.size === 0) return null;
  const out: EditSub[] = [];
  for (const sb of model.subs) {
    const nodes = sb.nodes.filter((n) => !gone.has(n)).map((n) => ({ ...n }));
    if (nodes.length === sb.nodes.length) {
      out.push({ ...sb, nodes });
      continue;
    }
    if (nodes.length < 2) continue;
    // Una forma cerrada de dos nodos sin curvas no encierra nada.
    if (sb.closed && nodes.length === 2 && !isCurve(nodes[0], nodes[1]) && !isCurve(nodes[1], nodes[0])) continue;
    out.push({ ...sb, nodes });
  }
  return { model: { subs: out }, sel: [] };
}

// ---- segmentos recta / curva ----------------------------------------------------------------------------------------

export function setSegmentsKind(
  model: EditModel,
  segs: readonly { s: number; i: number }[],
  kind: 'line' | 'curve',
  keep: readonly number[] = [],
): EditResult | null {
  if (segs.length === 0) return null;
  const m = cloneModel(model);
  for (const { s, i } of segs) {
    const sb = m.subs[s];
    if (!sb || i >= segmentCount(sb)) continue;
    const [a, b] = segmentEnds(sb, i);
    if (kind === 'line') {
      a.outH = null;
      b.inH = null;
    } else {
      // Manejadores a tercios: una recta convertida en curva conserva su forma.
      if (!a.outH) a.outH = lerpPoint(a.p, b.p, 1 / 3);
      if (!b.inH) b.inH = lerpPoint(a.p, b.p, 2 / 3);
    }
  }
  return { model: m, sel: [...keep] };
}

// ---- tipo de nodo ----------------------------------------------------------------------------------------------------

/**
 * Suave y simétrico alinean los manejadores (el simétrico, además, los iguala) y crean los que
 * falten con un tercio del segmento vecino. Esquina no mueve nada: solo deja de acoplar los manejadores
 * (quien llama lo recuerda con `positionKey`). Un extremo de trayecto abierto no tiene qué alinear.
 */
export function setNodeType(model: EditModel, sel: readonly number[], type: NodeType): EditResult | null {
  const m = cloneModel(model);
  const picked = refsOf(m, sel);
  if (picked.size === 0) return null;
  if (type !== 'corner') {
    for (const sb of m.subs) {
      const n = sb.nodes.length;
      sb.nodes.forEach((node, i) => {
        if (!picked.has(node)) return;
        const prev = i > 0 ? sb.nodes[i - 1] : sb.closed ? sb.nodes[n - 1] : null;
        const next = i < n - 1 ? sb.nodes[i + 1] : sb.closed ? sb.nodes[0] : null;
        if (!prev || !next) return;
        let dir: Point;
        let lenIn = dist(node.p, prev.p) / 3;
        let lenOut = dist(next.p, node.p) / 3;
        if (node.inH && node.outH) {
          dir = unit(sub(node.outH, node.inH), unit(sub(next.p, prev.p)));
          lenIn = dist(node.inH, node.p);
          lenOut = dist(node.outH, node.p);
        } else if (node.outH) {
          dir = unit(sub(node.outH, node.p));
          lenOut = dist(node.outH, node.p);
        } else if (node.inH) {
          dir = unit(sub(node.p, node.inH));
          lenIn = dist(node.inH, node.p);
        } else {
          dir = unit(sub(next.p, prev.p));
        }
        if (type === 'symmetric') lenIn = lenOut = (lenIn + lenOut) / 2;
        node.inH = sub(node.p, mul(dir, lenIn));
        node.outH = add(node.p, mul(dir, lenOut));
      });
    }
  }
  return { model: m, sel: [...sel] };
}

// ---- romper, unir y cerrar --------------------------------------------------------------------------------------------

function reverseSub(sb: EditSub): void {
  sb.nodes.reverse();
  for (const n of sb.nodes) [n.inH, n.outH] = [n.outH, n.inH];
}

/** Hay algún nodo seleccionado donde romper (no es un extremo de un trayecto abierto). */
export function canBreak(model: EditModel, sel: readonly number[]): boolean {
  const picked = refsOf(model, sel);
  return model.subs.some((sb) => sb.nodes.some((n, i) => picked.has(n) && (sb.closed || (i > 0 && i < sb.nodes.length - 1))));
}

/**
 * Parte el trayecto en cada nodo seleccionado: en cada corte queda un nodo al final de un trozo y
 * otro (en el mismo lugar) al principio del siguiente. Un trayecto cerrado se abre. La selección
 * resultante son los nodos que empiezan trozo tras un corte.
 */
export function breakAtNodes(model: EditModel, sel: readonly number[]): EditResult | null {
  const m = cloneModel(model);
  const picked = refsOf(m, sel);
  const starts = new Set<EditNode>();
  const out: EditSub[] = [];
  let did = false;
  for (const sb of m.subs) {
    let nodes = sb.nodes;
    let cuts: number[];
    if (sb.closed) {
      const first = nodes.findIndex((n) => picked.has(n));
      if (first < 0) {
        out.push(sb);
        continue;
      }
      // Se gira para empezar en el primer corte y se duplica ese nodo al final.
      const rotated = [...nodes.slice(first), ...nodes.slice(0, first)];
      const copy: EditNode = { p: rotated[0].p, inH: rotated[0].inH, outH: null };
      rotated[0].inH = null;
      rotated.push(copy);
      nodes = rotated;
      starts.add(rotated[0]);
      cuts = nodes.map((n, i) => (i > 0 && i < nodes.length - 1 && picked.has(n) ? i : -1)).filter((i) => i > 0);
    } else {
      cuts = nodes.map((n, i) => (i > 0 && i < nodes.length - 1 && picked.has(n) ? i : -1)).filter((i) => i > 0);
      if (cuts.length === 0) {
        out.push(sb);
        continue;
      }
    }
    did = true;
    let from = 0;
    for (const c of cuts) {
      const tail: EditNode = { p: nodes[c].p, inH: nodes[c].inH, outH: null };
      out.push({ nodes: [...nodes.slice(from, c), tail], closed: false, group: sb.group });
      nodes[c].inH = null;
      starts.add(nodes[c]);
      from = c;
    }
    out.push({ nodes: nodes.slice(from), closed: false, group: sb.group });
  }
  if (!did) return null;
  const model2 = { subs: out };
  return { model: model2, sel: indicesOf(model2, starts) };
}

/** Extremos de trayectos abiertos entre los nodos seleccionados. */
function endpointsOf(m: EditModel, picked: ReadonlySet<EditNode>): { s: number; first: boolean; node: EditNode }[] {
  const out: { s: number; first: boolean; node: EditNode }[] = [];
  m.subs.forEach((sb, s) => {
    if (sb.closed) return;
    const a = sb.nodes[0];
    const b = sb.nodes[sb.nodes.length - 1];
    if (picked.has(a)) out.push({ s, first: true, node: a });
    if (picked.has(b) && b !== a) out.push({ s, first: false, node: b });
  });
  return out;
}

/** ¿Hay exactamente dos extremos abiertos seleccionados que se puedan unir (o cerrar, si son del mismo trayecto)? */
export function canJoin(model: EditModel, sel: readonly number[]): boolean {
  const picked = refsOf(model, sel);
  return picked.size === 2 && endpointsOf(model, picked).length === 2;
}

/**
 * Une dos extremos abiertos. Si son los dos de un mismo trayecto, lo cierra; si son de trayectos
 * distintos, los junta en uno. Si los extremos coinciden se funden en un nodo; si no, se unen con
 * una recta y ninguno se mueve.
 */
export function joinEndpoints(model: EditModel, sel: readonly number[]): EditResult | null {
  const m = cloneModel(model);
  const picked = refsOf(m, sel);
  if (picked.size !== 2) return null;
  const ends = endpointsOf(m, picked);
  if (ends.length !== 2) return null;
  const [e1, e2] = ends;
  if (e1.s === e2.s) return closeSubs(model, [e1.s], sel);
  // A.final -> B.inicio
  let a = e1;
  let b = e2;
  if (a.first && !b.first) [a, b] = [b, a]; // el final de B conecta con el inicio de A
  const subA = m.subs[a.s];
  const subB = m.subs[b.s];
  if (a.first) reverseSub(subA);
  if (!b.first) reverseSub(subB);
  const la = subA.nodes[subA.nodes.length - 1];
  const fb = subB.nodes[0];
  const merged: EditNode[] = [...subA.nodes];
  let junction: EditNode[];
  if (near(la.p, fb.p)) {
    la.outH = fb.outH;
    junction = [la];
    merged.push(...subB.nodes.slice(1));
  } else {
    junction = [la, fb];
    merged.push(...subB.nodes);
  }
  const lo = Math.min(a.s, b.s);
  const subs = m.subs.filter((_, s) => s !== a.s && s !== b.s);
  subs.splice(lo, 0, { nodes: merged, closed: false, group: m.subs[lo].group });
  const model2 = { subs };
  return { model: model2, sel: indicesOf(model2, new Set(junction)) };
}

/** Cierra los trayectos abiertos indicados. Si sus extremos coinciden, los funde. */
export function closeSubs(model: EditModel, which: readonly number[], keep: readonly number[] = []): EditResult | null {
  const m = cloneModel(model);
  const kept = refsOf(m, keep);
  let did = false;
  for (const s of which) {
    const sb = m.subs[s];
    if (!sb || sb.closed || sb.nodes.length < 2) continue;
    const first = sb.nodes[0];
    const last = sb.nodes[sb.nodes.length - 1];
    if (sb.nodes.length >= 3 && near(first.p, last.p)) {
      first.inH = last.inH;
      sb.nodes.pop();
      if (kept.delete(last)) kept.add(first);
    }
    sb.closed = true;
    did = true;
  }
  if (!did) return null;
  return { model: m, sel: indicesOf(m, kept) };
}

/** Índices de los subtrayectos abiertos que tienen algún nodo seleccionado. */
export function openSubsOf(model: EditModel, sel: readonly number[]): number[] {
  const picked = refsOf(model, sel);
  const out: number[] = [];
  model.subs.forEach((sb, s) => {
    if (!sb.closed && sb.nodes.some((n) => picked.has(n))) out.push(s);
  });
  return out;
}

// ---- mover -------------------------------------------------------------------------------------------------------------

/** Mueve los nodos seleccionados (con sus manejadores) un desplazamiento local. */
export function moveNodes(model: EditModel, sel: readonly number[], d: Point): EditModel {
  const m = cloneModel(model);
  for (const n of refsOf(m, sel)) {
    n.p = add(n.p, d);
    if (n.inH) n.inH = add(n.inH, d);
    if (n.outH) n.outH = add(n.outH, d);
  }
  return m;
}

/**
 * Mueve un manejador a `to` (coordenadas locales). Con un nodo suave o simétrico el otro manejador
 * lo acompaña manteniendo la línea (el simétrico, también la longitud); con `independent` o en una esquina
 * se mueve solo.
 */
export function moveHandle(model: EditModel, k: number, which: 'in' | 'out', to: Point, type: NodeType, independent = false): EditModel {
  const m = cloneModel(model);
  const n = nodeAt(m, k);
  if (!n) return m;
  const key = which === 'in' ? 'inH' : 'outH';
  const other = which === 'in' ? 'outH' : 'inH';
  n[key] = to;
  if (!independent && type !== 'corner' && n[other]) {
    const d = unit(sub(n.p, to), [1, 0]);
    const l = type === 'symmetric' ? dist(to, n.p) : dist(n[other]!, n.p);
    n[other] = add(n.p, mul(d, l));
  }
  return m;
}

// ---- consulta geométrica -----------------------------------------------------------------------------------------------

/** Puntos de un segmento del modelo llevados por `t` (coordenadas del documento). */
function segmentPoints(a: EditNode, b: EditNode, t: Affine): [Point, Point, Point, Point] {
  return [applyPoint(t, a.p), applyPoint(t, a.outH ?? a.p), applyPoint(t, b.inH ?? b.p), applyPoint(t, b.p)];
}

export interface SegmentHit {
  s: number;
  i: number;
  t: number;
  /** Distancia (mm de documento) del punto a la curva. */
  dist: number;
}

/** Segmento más cercano a `p` (coordenadas del documento, con el `transform` del objeto), si está a menos de `tol`. */
export function nearestSegment(model: EditModel, t: Affine, p: Point, tol: number): SegmentHit | null {
  let best: SegmentHit | null = null;
  model.subs.forEach((sb, s) => {
    for (let i = 0; i < segmentCount(sb); i++) {
      const [a, b] = segmentEnds(sb, i);
      const [p0, c1, c2, p1] = segmentPoints(a, b, t);
      const minX = Math.min(p0[0], c1[0], c2[0], p1[0]) - tol;
      const maxX = Math.max(p0[0], c1[0], c2[0], p1[0]) + tol;
      const minY = Math.min(p0[1], c1[1], c2[1], p1[1]) - tol;
      const maxY = Math.max(p0[1], c1[1], c2[1], p1[1]) + tol;
      if (p[0] < minX || p[0] > maxX || p[1] < minY || p[1] > maxY) continue;
      const r = isCurve(a, b) ? nearestOnCubic(p0, c1, c2, p1, p) : nearestOnLine(p0, p1, p);
      if (r.dist <= tol && (!best || r.dist < best.dist)) best = { s, i, t: r.t, dist: r.dist };
    }
  });
  return best;
}

function nearestOnLine(a: Point, b: Point, p: Point): { t: number; dist: number } {
  const d = sub(b, a);
  const l2 = d[0] * d[0] + d[1] * d[1];
  const t = l2 === 0 ? 0 : Math.min(1, Math.max(0, ((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / l2));
  return { t, dist: dist(lerpPoint(a, b, t), p) };
}

function nearestOnCubic(p0: Point, c1: Point, c2: Point, p1: Point, p: Point): { t: number; dist: number } {
  const N = 32;
  let bestT = 0;
  let bestD = Infinity;
  for (let k = 0; k <= N; k++) {
    const d = dist(cubicAt(p0, c1, c2, p1, k / N), p);
    if (d < bestD) {
      bestD = d;
      bestT = k / N;
    }
  }
  // Se afina con búsqueda de sección áurea alrededor del mejor muestreo.
  let lo = Math.max(0, bestT - 1 / N);
  let hi = Math.min(1, bestT + 1 / N);
  const g = (Math.sqrt(5) - 1) / 2;
  for (let it = 0; it < 24; it++) {
    const x1 = hi - g * (hi - lo);
    const x2 = lo + g * (hi - lo);
    if (dist(cubicAt(p0, c1, c2, p1, x1), p) < dist(cubicAt(p0, c1, c2, p1, x2), p)) hi = x2;
    else lo = x1;
  }
  const t = (lo + hi) / 2;
  const d = dist(cubicAt(p0, c1, c2, p1, t), p);
  return d <= bestD ? { t, dist: d } : { t: bestT, dist: bestD };
}

// ---- esquinas recordadas ---------------------------------------------------------------------------------------

/** Mueve las claves de posición de los nodos desplazados (para que "esquina" los siga). */
export function rekeyCorners(corners: readonly string[], from: readonly Point[], to: readonly Point[]): string[] {
  if (corners.length === 0) return [];
  const map = new Map<string, string>();
  from.forEach((p, i) => map.set(positionKey(p), positionKey(to[i])));
  return corners.map((k) => map.get(k) ?? k);
}

/** Posiciones locales de los nodos indicados. */
export function positionsOf(model: EditModel, sel: readonly number[]): Point[] {
  const out: Point[] = [];
  for (const k of sel) {
    const n = nodeAt(model, k);
    if (n) out.push(n.p);
  }
  return out;
}
