import { flattenBez, polylineBez, type BezPath, type FlatPath } from '@/lib/bezpath';
import { objectCurves, pointsBounds, type Bounds } from '@/lib/geometry';
import type { LaserApi } from '../api';
import type { DesignObject, Point, ShapeOpArgs, ShapeOpResult, ShapeParams, ShapeStats } from '../types';

type Lag = <T>(value: () => T) => Promise<T>;

/**
 * Versión simulada de `shape_op` (ver `src-tauri/src/edit/shape_ops.rs`) para `npm run dev` sin el
 * motor. Sigue las mismas reglas de resultado (lista completa de objetos, los cambiados con id nuevo
 * `{op}-N`); las simplificaciones están anotadas en cada operación.
 */

const EPS = 1e-7;
const dist = (a: Point, b: Point) => Math.hypot(a[0] - b[0], a[1] - b[1]);
const emptyStats = (): ShapeStats => ({ count: 0, skipped: 0, clamped: 0, produced: 0, found: 0, nodes_before: 0, nodes_after: 0, widths: [], removed: [] });

// --- trazos ---------------------------------------------------------------------------------------

/** Parte una trayectoria en subtrayectos (cada uno con al menos un segmento dibujado). */
function splitSubs(path: BezPath): BezPath[] {
  const out: BezPath[] = [];
  let cur: BezPath = [];
  let drawn = false;
  const flush = () => {
    if (drawn && cur.length > 0) out.push(cur);
    cur = [];
    drawn = false;
  };
  for (const s of path) {
    if (s.type === 'move_to') {
      flush();
      cur.push(s);
    } else if (s.type === 'close') {
      if (cur.length > 0) {
        cur.push(s);
        flush();
      }
    } else if (cur.length > 0) {
      cur.push(s);
      drawn = true;
    }
  }
  flush();
  return out;
}

const subsOf = (o: DesignObject): BezPath[] => objectCurves(o).flatMap(splitSubs);
const flatOf = (sub: BezPath): FlatPath => flattenBez(sub)[0] ?? { points: [], closed: false };

/** Nodos de un subtrayecto: el inicio y el destino de cada segmento, sin repetir el cierre. */
function nodeCount(sub: BezPath): number {
  let n = 0;
  let start: Point | null = null;
  let last: Point | null = null;
  let closed = false;
  for (const s of sub) {
    if (s.type === 'move_to') {
      start = s.to;
      last = s.to;
      n++;
    } else if (s.type === 'close') closed = true;
    else {
      last = s.to;
      n++;
    }
  }
  return closed && n > 1 && start && last && dist(start, last) < 1e-9 ? n - 1 : n;
}

function makeObject(src: DesignObject, id: string, paths: BezPath[]): DesignObject {
  const out: DesignObject = { ...src, id, transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths }, locked: false };
  // La forma paramétrica, el punto de inicio, los puentes y las copias virtuales dejan de valer.
  delete out.shape;
  delete out.start;
  delete out.manual_tabs;
  delete out.array;
  return out;
}

function idMaker(prefix: string, objects: DesignObject[]) {
  const taken = new Set(objects.map((o) => o.id));
  let n = 0;
  return () => {
    let id: string;
    do id = `${prefix}-${++n}`;
    while (taken.has(id));
    return id;
  };
}

type Slot = 'keep' | 'gone' | DesignObject[];

function finish(objects: DesignObject[], slots: Slot[], stats: ShapeStats): ShapeOpResult {
  const out: DesignObject[] = [];
  objects.forEach((o, i) => {
    const s = slots[i]!;
    if (s === 'keep') out.push(structuredClone(o));
    else if (s !== 'gone') {
      stats.produced += s.length;
      out.push(...s);
    }
  });
  return { objects: out, stats };
}

// --- recortar ---------------------------------------------------------------------------------------

const segCount = (p: FlatPath) => (p.points.length < 2 ? 0 : p.closed ? p.points.length : p.points.length - 1);
const segOf = (p: FlatPath, i: number): [Point, Point] => [p.points[i % p.points.length]!, p.points[(i + 1) % p.points.length]!];

function pointAt(p: FlatPath, param: number): Point {
  const n = segCount(p);
  const q = p.closed ? ((param % n) + n) % n : Math.min(Math.max(param, 0), n);
  const i = Math.min(Math.floor(q), n - 1);
  const [a, b] = segOf(p, i);
  const t = q - i;
  return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
}

/** Recorrido de `len` (en parámetro) desde `from`, sin puntos repetidos; `null` si queda en un punto. */
function subPath(p: FlatPath, from: number, len: number): FlatPath | null {
  const n = segCount(p);
  const pts: Point[] = [pointAt(p, from)];
  for (let k = Math.floor(from + 1e-12) + 1; k < from + len - 1e-12; k++) pts.push(p.points[p.closed ? ((k % n) + n) % n : Math.min(k, p.points.length - 1)]!);
  pts.push(pointAt(p, from + len));
  const clean = pts.filter((q, i) => i === 0 || dist(q, pts[i - 1]!) > 1e-9);
  return clean.length >= 2 ? { points: clean, closed: false } : null;
}

const cross = (a: Point, b: Point) => a[0] * b[1] - a[1] * b[0];

/** Valores t (0 a 1) sobre `p-q` donde corta o se solapa con `a-b`. */
function segmentCuts(p: Point, q: Point, a: Point, b: Point): number[] {
  const d1: Point = [q[0] - p[0], q[1] - p[1]];
  const d2: Point = [b[0] - a[0], b[1] - a[1]];
  const [l1, l2] = [Math.hypot(...d1), Math.hypot(...d2)];
  if (l1 <= EPS || l2 <= EPS) return [];
  const ap: Point = [a[0] - p[0], a[1] - p[1]];
  const denom = cross(d1, d2);
  if (Math.abs(denom) > 1e-9 * l1 * l2) {
    const t = cross(ap, d2) / denom;
    const u = cross(ap, d1) / denom;
    return t >= -EPS / l1 && t <= 1 + EPS / l1 && u >= -EPS / l2 && u <= 1 + EPS / l2 ? [Math.min(1, Math.max(0, t))] : [];
  }
  if (Math.abs(cross(ap, d1)) / l1 > EPS) return [];
  return [a, b].map((e) => ((e[0] - p[0]) * d1[0] + (e[1] - p[1]) * d1[1]) / (l1 * l1)).filter((t) => t >= 0 && t <= 1);
}

function cutParams(path: FlatPath, own: number, all: FlatPath[]): number[] {
  const n = segCount(path);
  const found: number[] = [];
  for (let i = 0; i < n; i++) {
    const [p, q] = segOf(path, i);
    all.forEach((other, k) => {
      for (let j = 0; j < segCount(other); j++) {
        if (k === own) {
          const d = Math.abs(i - j);
          if (d <= 1 || (path.closed && d === n - 1)) continue;
        }
        const [a, b] = segOf(other, j);
        for (const t of segmentCuts(p, q, a, b)) found.push(i + t);
      }
    });
  }
  found.sort((x, y) => x - y);
  const out: number[] = [];
  let lastPoint: Point | null = null;
  for (const t of found) {
    const pt = pointAt(path, t);
    if (!lastPoint || dist(lastPoint, pt) > EPS) out.push(path.closed ? t % n : t);
    lastPoint = pt;
  }
  return path.closed ? [...new Set(out)].sort((x, y) => x - y) : out;
}

function distToSeg(p: Point, a: Point, b: Point): [number, number] {
  const [dx, dy] = [b[0] - a[0], b[1] - a[1]];
  const l2 = dx * dx + dy * dy;
  const t = l2 === 0 ? 0 : Math.min(1, Math.max(0, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2));
  return [dist(p, [a[0] + dx * t, a[1] + dy * t]), t];
}

function mockTrim(objects: DesignObject[], params: ShapeParams): ShapeOpResult {
  if (!params.point) throw new Error('Falta el punto donde recortar.');
  const click = params.point;
  const subs: { owner: number; sub: BezPath; flat: FlatPath }[] = [];
  objects.forEach((o, owner) => subsOf(o).forEach((sub) => subs.push({ owner, sub, flat: flatOf(sub) })));
  let best: { k: number; param: number; d: number } | null = null;
  subs.forEach(({ flat }, k) => {
    for (let i = 0; i < segCount(flat); i++) {
      const [a, b] = segOf(flat, i);
      const [d, t] = distToSeg(click, a, b);
      if (!best || d < best.d) best = { k, param: i + t, d };
    }
  });
  const hit = best as { k: number; param: number; d: number } | null;
  if (!hit || (params.max_dist !== undefined && hit.d > params.max_dist)) throw new Error('No hay ningún trazo cerca del punto indicado.');
  const flats = subs.map((s) => s.flat);
  const path = flats[hit.k]!;
  const n = segCount(path);
  const cuts = cutParams(path, hit.k, flats);
  let removed: FlatPath | null;
  const remaining: FlatPath[] = [];
  if (path.closed) {
    if (cuts.length < 2) removed = path;
    else {
      const idx = cuts.findLastIndex((c) => c <= hit.param + 1e-12);
      const [prev, next] = idx === -1 ? [cuts[cuts.length - 1]! - n, cuts[0]!] : idx + 1 < cuts.length ? [cuts[idx]!, cuts[idx + 1]!] : [cuts[idx]!, cuts[0]! + n];
      removed = subPath(path, prev, next - prev);
      const rest = subPath(path, next, n - (next - prev));
      if (rest) remaining.push(rest);
    }
  } else {
    const prev = cuts.findLast((c) => c <= hit.param + 1e-12) ?? 0;
    const next = cuts.find((c) => c > hit.param + 1e-12) ?? n;
    removed = prev <= 0 && next >= n ? path : subPath(path, prev, next - prev);
    if (prev > 0) {
      const a = subPath(path, 0, prev);
      if (a) remaining.push(a);
    }
    if (next < n) {
      const b = subPath(path, next, n - next);
      if (b) remaining.push(b);
    }
  }
  if (!removed) throw new Error('No hay ningún trazo cerca del punto indicado.');
  const owner = subs[hit.k]!.owner;
  const next = idMaker('trim', objects);
  const paths = subs.flatMap((s, k) => (s.owner !== owner ? [] : k === hit.k ? remaining.map((r) => polylineBez(r.points, false)) : [s.sub]));
  const fresh: DesignObject[] = [];
  if (paths.length > 0) fresh.push(makeObject(objects[owner]!, next(), paths));
  if (params.keep_removed) fresh.push(makeObject(objects[owner]!, next(), [polylineBez(removed.points, removed.closed)]));
  const slots: Slot[] = objects.map((): Slot => 'keep');
  slots[owner] = fresh.length > 0 ? fresh : 'gone';
  return finish(objects, slots, { ...emptyStats(), count: 1, removed: removed.points });
}

// --- separar ----------------------------------------------------------------------------------------

const polyArea = (pts: Point[]) => Math.abs(pts.reduce((s, p, i) => s + cross(p, pts[(i + 1) % pts.length]!), 0) / 2);

function inside(pt: Point, poly: Point[]): boolean {
  let c = false;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const [a, b] = [poly[i]!, poly[j]!];
    if (a[1] > pt[1] !== b[1] > pt[1] && pt[0] < ((b[0] - a[0]) * (pt[1] - a[1])) / (b[1] - a[1]) + a[0]) c = !c;
  }
  return c;
}

/** Grupos de índices: cada exterior con los huecos que contiene de forma inmediata (si `keepHoles`). */
function breakGroups(flats: FlatPath[], keepHoles: boolean): number[][] {
  if (!keepHoles) return flats.map((_, i) => [i]);
  const area = flats.map((f) => (f.closed ? polyArea(f.points) : 0));
  const parent = flats.map((f, i) => {
    if (!f.closed || f.points.length < 3) return -1;
    let best = -1;
    flats.forEach((g, j) => {
      if (j !== i && g.closed && area[j]! > area[i]! && inside(f.points[0]!, g.points) && (best === -1 || area[j]! < area[best]!)) best = j;
    });
    return best;
  });
  const depth = (i: number): number => (parent[i] === -1 ? 0 : 1 + depth(parent[i]!));
  const groupOf: number[] = [];
  const groups: number[][] = [];
  [...flats.keys()]
    .sort((a, b) => depth(a) - depth(b))
    .forEach((i) => {
      const p = parent[i]!;
      if (depth(i) % 2 === 1 && p !== -1 && groupOf[p] !== undefined) {
        groupOf[i] = groupOf[p]!;
        groups[groupOf[p]!]!.push(i);
      } else {
        groupOf[i] = groups.length;
        groups.push([i]);
      }
    });
  return groups.sort((a, b) => Math.min(...a) - Math.min(...b));
}

function mockBreak(objects: DesignObject[], params: ShapeParams): ShapeOpResult {
  const next = idMaker('break', objects);
  const slots: Slot[] = objects.map((): Slot => 'keep');
  const stats = emptyStats();
  objects.forEach((o, i) => {
    const subs = subsOf(o);
    if (subs.length < 2) return;
    const groups = breakGroups(subs.map(flatOf), params.keep_holes ?? false);
    if (groups.length < 2) return;
    stats.count++;
    slots[i] = groups.map((g) => makeObject(o, next(), g.map((k) => subs[k]!)));
  });
  return finish(objects, slots, stats);
}

// --- optimizar --------------------------------------------------------------------------------------

function douglasPeucker(pts: Point[], tol: number): Point[] {
  if (pts.length < 3) return pts;
  const [a, b] = [pts[0]!, pts[pts.length - 1]!];
  let worst = 0;
  let at = 0;
  for (let i = 1; i < pts.length - 1; i++) {
    const d = distToSeg(pts[i]!, a, b)[0];
    if (d > worst) {
      worst = d;
      at = i;
    }
  }
  if (worst <= tol) return [a, b];
  return [...douglasPeucker(pts.slice(0, at + 1), tol).slice(0, -1), ...douglasPeucker(pts.slice(at), tol)];
}

/**
 * Simplifica con Douglas-Peucker entre las esquinas (giro mayor que `corner`). Simplificación del
 * mock: `smooth` no ajusta curvas (el motor real sí), solo simplifica rectas.
 */
function simplifyFlat(f: FlatPath, tol: number, corner: number): Point[] {
  const clean = f.points.filter((p, i) => i === 0 || dist(p, f.points[i - 1]!) > 1e-6);
  const n = clean.length;
  if (tol <= 0 || n <= (f.closed ? 3 : 2)) return clean;
  const turn = (i: number) => {
    const [a, p, b] = [clean[(i + n - 1) % n]!, clean[i]!, clean[(i + 1) % n]!];
    const [u, v] = [Math.atan2(p[1] - a[1], p[0] - a[0]), Math.atan2(b[1] - p[1], b[0] - p[0])];
    return Math.abs(((v - u + 3 * Math.PI) % (2 * Math.PI)) - Math.PI) * (180 / Math.PI);
  };
  let anchors = clean.flatMap((_, i) => (!f.closed && (i === 0 || i === n - 1)) || (turn(i) > corner && (f.closed || (i > 0 && i < n - 1))) ? [i] : []);
  if (f.closed && anchors.length < 2) {
    const far = clean.reduce((bi, p, i) => (dist(p, clean[0]!) > dist(clean[bi]!, clean[0]!) ? i : bi), 0);
    anchors = [...new Set([0, far, ...anchors])].sort((a, b) => a - b);
  }
  const out: Point[] = [];
  const spans = f.closed ? anchors.map((a, k) => [a, anchors[(k + 1) % anchors.length]!] as const) : anchors.slice(1).map((b, k) => [anchors[k]!, b] as const);
  for (const [a, b] of spans) {
    const run: Point[] = [clean[a]!];
    for (let i = (a + 1) % n; ; i = (i + 1) % n) {
      run.push(clean[i]!);
      if (i === b) break;
    }
    out.push(...douglasPeucker(run, tol).slice(0, -1));
  }
  if (!f.closed) out.push(clean[n - 1]!);
  return out;
}

function mockOptimize(objects: DesignObject[], params: ShapeParams): ShapeOpResult {
  const tol = params.tolerance ?? 0.02;
  if (!Number.isFinite(tol) || tol < 0) throw new Error('La tolerancia no es un número válido (debe ser 0 o más).');
  const next = idMaker('opt', objects);
  const slots: Slot[] = objects.map((): Slot => 'keep');
  const stats = emptyStats();
  objects.forEach((o, i) => {
    let changed = false;
    const paths = subsOf(o).map((sub) => {
      const before = nodeCount(sub);
      stats.nodes_before += before;
      const f = flatOf(sub);
      const pts = simplifyFlat(f, tol, params.corner_angle ?? 35);
      const optimized = polylineBez(pts, f.closed);
      const after = nodeCount(optimized);
      if (after < before) {
        stats.nodes_after += after;
        stats.count++;
        changed = true;
        return optimized;
      }
      stats.nodes_after += before;
      return sub;
    });
    if (changed) slots[i] = [makeObject(o, next(), paths)];
  });
  return finish(objects, slots, stats);
}

// --- redondear esquinas -----------------------------------------------------------------------------

/**
 * Esquinas de recta con recta: redondeo con cúbica, chaflán o mordisco. Solo para subtrayectos hechos
 * de rectas (el mock no toca los que tienen curvas). Reparte el espacio con la mitad de cada recta.
 */
function filletPoints(pts: Point[], closed: boolean, radius: number, style: string, only: Point[] | undefined, stats: ShapeStats): BezPath {
  const n = pts.length;
  const out: BezPath = [];
  const corner = (i: number) => {
    if (!closed && (i === 0 || i === n - 1)) return null;
    const v = pts[i]!;
    if (only && !only.some((p) => dist(p, v) <= 0.01)) return null;
    const [a, b] = [pts[(i + n - 1) % n]!, pts[(i + 1) % n]!];
    const [la, lb] = [dist(a, v), dist(b, v)];
    if (la < EPS || lb < EPS) return null;
    const u: Point = [(a[0] - v[0]) / la, (a[1] - v[1]) / la];
    const w: Point = [(b[0] - v[0]) / lb, (b[1] - v[1]) / lb];
    const th = Math.acos(Math.min(1, Math.max(-1, u[0] * w[0] + u[1] * w[1])));
    if (th > Math.PI - 0.0175 || th < 1e-6) return null;
    let d = style === 'round' ? radius / Math.tan(th / 2) : radius;
    const cap = Math.min(la, lb) / (closed || (i > 1 && i < n - 2) ? 2 : 1);
    if (d > cap + 1e-9) {
      if (style !== 'round' && d > Math.min(la, lb) + 1e-9) {
        stats.skipped++;
        return null;
      }
      d = Math.min(d, cap);
      stats.clamped++;
    }
    return { v, u, w, th, d };
  };
  const corners = pts.map((_, i) => corner(i));
  const start = corners[0] ? [corners[0].v[0] + corners[0].w[0] * corners[0].d, corners[0].v[1] + corners[0].w[1] * corners[0].d] as Point : pts[0]!;
  out.push({ type: 'move_to', to: start });
  for (let k = 1; k <= (closed ? n : n - 1); k++) {
    const i = k % n;
    const c = corners[i];
    if (!c) {
      if (!(closed && i === 0)) out.push({ type: 'line_to', to: pts[i]! });
      continue;
    }
    const t1: Point = [c.v[0] + c.u[0] * c.d, c.v[1] + c.u[1] * c.d];
    const t2: Point = [c.v[0] + c.w[0] * c.d, c.v[1] + c.w[1] * c.d];
    out.push({ type: 'line_to', to: t1 });
    stats.count++;
    if (style === 'chamfer') out.push({ type: 'line_to', to: t2 });
    else if (style === 'round') {
      const h = (4 / 3) * Math.tan((Math.PI - c.th) / 4) * c.d * Math.tan(c.th / 2);
      out.push({ type: 'cubic_to', ctrl1: [t1[0] - c.u[0] * h, t1[1] - c.u[1] * h], ctrl2: [t2[0] - c.w[0] * h, t2[1] - c.w[1] * h], to: t2 });
    } else {
      // Mordisco: arco centrado en la esquina, aproximado con una cúbica.
      const h = (4 / 3) * Math.tan(c.th / 4) * c.d;
      const side = Math.sign(cross(c.u, c.w)) || 1;
      out.push({ type: 'cubic_to', ctrl1: [t1[0] + side * c.u[1] * h, t1[1] - side * c.u[0] * h], ctrl2: [t2[0] - side * c.w[1] * h, t2[1] + side * c.w[0] * h], to: t2 });
    }
  }
  if (closed) out.push({ type: 'close' });
  return out;
}

function mockFillet(objects: DesignObject[], params: ShapeParams): ShapeOpResult {
  const radius = params.radius;
  if (radius === undefined) throw new Error('Falta el radio.');
  if (!Number.isFinite(radius) || radius <= 0) throw new Error('El radio debe ser un número mayor que 0.');
  const next = idMaker('fillet', objects);
  const slots: Slot[] = objects.map((): Slot => 'keep');
  const stats = emptyStats();
  objects.forEach((o, i) => {
    let changed = false;
    const paths = subsOf(o).map((sub) => {
      const straight = sub.every((s) => s.type === 'move_to' || s.type === 'line_to' || s.type === 'close');
      const f = flatOf(sub);
      if (!straight || f.points.length < 3) return sub;
      const before = stats.count;
      const result = filletPoints(f.points, f.closed, radius, params.style ?? 'round', params.nodes, stats);
      if (stats.count === before) return sub;
      changed = true;
      return result;
    });
    if (changed) slots[i] = [makeObject(o, next(), paths)];
  });
  return finish(objects, slots, stats);
}

// --- cortar formas ----------------------------------------------------------------------------------

function boundsRect(src: DesignObject, id: string, b: Bounds): DesignObject {
  return makeObject(src, id, [polylineBez([b[0], [b[1][0], b[0][1]], b[1], [b[0][0], b[1][1]]], true)]);
}

/**
 * Con cajas envolventes, como el resto del mock de geometría: cada forma se parte en la parte de su caja
 * que cubre la cuchilla (dentro) y hasta cuatro franjas (fuera).
 */
function mockCut(objects: DesignObject[], params: ShapeParams): ShapeOpResult {
  if (objects.length < 2) throw new Error('Cortar formas necesita la cuchilla y al menos una forma más.');
  const knifeIndex = params.knife_id ? objects.findIndex((o) => o.id === params.knife_id) : objects.length - 1;
  if (knifeIndex < 0) throw new Error(`La cuchilla «${params.knife_id}» no está entre los objetos.`);
  const knife = objects[knifeIndex]!;
  const kPaths = objectCurves(knife).flatMap((c) => flattenBez(c));
  if (kPaths.length === 0 || kPaths.some((p) => !p.closed)) throw new Error(`La cuchilla «${knife.id}» debe ser una forma cerrada. Ciérrala antes (por ejemplo con «Cerrar polilínea»).`);
  const kb = pointsBounds(kPaths.flatMap((p) => p.points))!;
  const next = idMaker('cut', objects);
  const slots: Slot[] = objects.map((): Slot => 'keep');
  const stats = emptyStats();
  objects.forEach((o, i) => {
    if (i === knifeIndex) return;
    const b = pointsBounds(objectCurves(o).flatMap((c) => flattenBez(c)).flatMap((p) => p.points));
    if (!b) return;
    const cut: Bounds = [[Math.max(b[0][0], kb[0][0]), Math.max(b[0][1], kb[0][1])], [Math.min(b[1][0], kb[1][0]), Math.min(b[1][1], kb[1][1])]];
    if (cut[1][0] - cut[0][0] <= EPS || cut[1][1] - cut[0][1] <= EPS) {
      stats.skipped++;
      return;
    }
    const parts: Bounds[] = [cut];
    if (cut[0][0] > b[0][0]) parts.push([[b[0][0], b[0][1]], [cut[0][0], b[1][1]]]);
    if (cut[1][0] < b[1][0]) parts.push([[cut[1][0], b[0][1]], [b[1][0], b[1][1]]]);
    if (cut[0][1] > b[0][1]) parts.push([[cut[0][0], b[0][1]], [cut[1][0], cut[0][1]]]);
    if (cut[1][1] < b[1][1]) parts.push([[cut[0][0], cut[1][1]], [cut[1][0], b[1][1]]]);
    if (parts.length < 2) {
      stats.skipped++;
      return;
    }
    stats.count++;
    slots[i] = parts.map((p) => boundsRect(o, next(), p));
  });
  if (stats.count > 0 && !params.keep_knife) slots[knifeIndex] = 'gone';
  return finish(objects, slots, stats);
}

// --- ranuras ----------------------------------------------------------------------------------------

/**
 * Solo agujeros rectangulares (rectángulos de 4 puntos dentro de la caja de otro trazo): cambia el
 * lado que coincide con el ancho buscado. El motor real reconoce además muescas y pestañas del borde.
 */
function mockSlots(objects: DesignObject[], params: ShapeParams): ShapeOpResult {
  const { from_width: from, to_width: to } = params;
  if (from === undefined) throw new Error('Falta el ancho actual de las ranuras.');
  if (to === undefined) throw new Error('Falta el ancho nuevo de las ranuras.');
  for (const [v, name] of [[from, 'actual'], [to, 'nuevo']] as const) {
    if (!Number.isFinite(v) || v <= 0) throw new Error(`El ancho ${name} debe ser un número mayor que 0.`);
  }
  const tol = params.tolerance ?? 0.1;
  const anchor = params.anchor ?? 'center';
  const [lo, hi] = anchor === 'center' ? [0.5, 0.5] : anchor === 'start' ? [0, 1] : [1, 0];
  const all = objects.flatMap((o, owner) => objectCurves(o).flatMap((c) => flattenBez(c)).map((flat) => ({ owner, flat })));
  const boxes = all.map(({ flat }) => pointsBounds(flat.points)!);
  const stats = emptyStats();
  const widths = new Map<number, number>();
  const changed = new Map<FlatPath, FlatPath>();
  all.forEach(({ flat }, k) => {
    const pts = flat.points;
    if (!flat.closed || pts.length !== 4) return;
    const e = [0, 1, 2, 3].map((i) => [pts[(i + 1) % 4]![0] - pts[i]![0], pts[(i + 1) % 4]![1] - pts[i]![1]] as Point);
    const rect = e.every((v, i) => Math.abs(v[0] * e[(i + 1) % 4]![0] + v[1] * e[(i + 1) % 4]![1]) < 1e-3 * Math.hypot(...v) * Math.hypot(...e[(i + 1) % 4]!));
    const b = boxes[k]!;
    const holed = boxes.some((o, j) => j !== k && o[0][0] < b[0][0] && o[0][1] < b[0][1] && o[1][0] > b[1][0] && o[1][1] > b[1][1]);
    if (!rect || !holed) return;
    const moved = pts.map((p) => [...p] as Point);
    const dims: [number, [number, number], [number, number]][] = [[0, [0, 3], [1, 2]], [1, [0, 1], [3, 2]]];
    for (const [ei, low, high] of dims) {
      const len = Math.hypot(...e[ei]!);
      const r = Math.round(len * 100) / 100;
      widths.set(r, (widths.get(r) ?? 0) + 1);
      if (Math.abs(len - from) > tol + 1e-9) continue;
      const u: Point = [e[ei]![0] / len, e[ei]![1] / len];
      const change = to - len;
      stats.found++;
      for (const [idxs, amount] of [[low, -change * lo], [high, change * hi]] as const) for (const idx of idxs) moved[idx] = [moved[idx]![0] + u[0] * amount, moved[idx]![1] + u[1] * amount];
      stats.count++;
    }
    if (moved.some((m, i) => dist(m, pts[i]!) > 1e-9)) changed.set(flat, { points: moved, closed: true });
  });
  stats.widths = [...widths.entries()].sort((a, b) => a[0] - b[0]);
  const next = idMaker('slot', objects);
  const slots: Slot[] = objects.map((): Slot => 'keep');
  objects.forEach((o, i) => {
    const mine = all.filter((a) => a.owner === i);
    if (mine.some((m) => changed.has(m.flat))) slots[i] = [makeObject(o, next(), mine.map((m) => { const f = changed.get(m.flat) ?? m.flat; return polylineBez(f.points, f.closed); }))];
  });
  return finish(objects, slots, stats);
}

export function mockShapeOp({ op, objects, params = {} }: ShapeOpArgs): ShapeOpResult {
  if (objects.length === 0) throw new Error('No hay objetos sobre los que operar.');
  const bad = objects.filter((o) => o.kind.type !== 'vector').map((o) => `«${o.id}»`);
  if (bad.length > 0) throw new Error(`Esta operación solo trabaja con trazos vectoriales; convierte los textos a trayecto y quita las imágenes: ${bad.join(', ')}.`);
  switch (op) {
    case 'trim':
      return mockTrim(objects, params);
    case 'break_apart':
      return mockBreak(objects, params);
    case 'optimize':
      return mockOptimize(objects, params);
    case 'fillet':
      return mockFillet(objects, params);
    case 'cut_shapes':
      return mockCut(objects, params);
    case 'resize_slots':
      return mockSlots(objects, params);
  }
}

/** Métodos simulados de las operaciones de forma. */
export function mockShapes(lag: Lag): Pick<LaserApi, 'shapeOp'> {
  return { shapeOp: (args) => lag(() => mockShapeOp(args)) };
}
