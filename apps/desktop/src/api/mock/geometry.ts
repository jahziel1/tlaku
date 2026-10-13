import { roundedRectBez, polylineBez } from '@/lib/bezpath';
import { objectPolylines, pointsBounds, unionBounds, type Bounds } from '@/lib/geometry';
import type { Point } from '../types';
import type { LaserApi } from '../api';
import type { DesignObject, GeometryOpArgs } from '../types';

type Lag = <T>(value: () => T) => Promise<T>;

const list = (ids: string[]) => ids.map((id) => `«${id}»`).join(', ');

/** Misma validación que el motor: sin imágenes, sin contornos vacíos y sin formas abiertas. */
function boundsOfOperands(objects: DesignObject[]): { id: string; layer: string; bounds: Bounds }[] {
  const images = objects.filter((o) => o.kind.type === 'image').map((o) => o.id);
  if (images.length) throw new Error(`Las imágenes no pueden participar en esta operación: ${list(images)}.`);
  const flat = objects.map((o) => ({ o, paths: objectPolylines(o) }));
  const open = flat.filter((f) => f.paths.some((p) => !p.closed)).map((f) => f.o.id);
  if (open.length) {
    throw new Error(`Hay formas abiertas que no pueden participar: ${list(open)}. Ciérralas antes (por ejemplo con «Cerrar polilínea»).`);
  }
  const empty = flat.filter((f) => f.paths.length === 0).map((f) => f.o.id);
  if (empty.length) throw new Error(`Estos objetos no tienen contorno que operar (¿un texto sin contorno generado?): ${list(empty)}.`);
  return flat.map((f) => ({ id: f.o.id, layer: f.o.layer, bounds: pointsBounds(f.paths.flatMap((p) => p.points))! }));
}

function rectObject(id: string, layer: string, [[x0, y0], [x1, y1]]: Bounds, radius = 0): DesignObject {
  const path = radius > 0 ? roundedRectBez(x0, y0, x1, y1, radius) : polylineBez([[x0, y0], [x1, y0], [x1, y1], [x0, y1]], true);
  return { id, layer, transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths: [path] }, locked: false, hidden: false, group: null };
}

const area = (b: Bounds) => Math.max(0, b[1][0] - b[0][0]) * Math.max(0, b[1][1] - b[0][1]);

/** Intersección de cajas, o `null` si no se tocan. */
function intersect(a: Bounds, b: Bounds): Bounds | null {
  const r: Bounds = [[Math.max(a[0][0], b[0][0]), Math.max(a[0][1], b[0][1])], [Math.min(a[1][0], b[1][0]), Math.min(a[1][1], b[1][1])]];
  return area(r) > 0 ? r : null;
}

/** A menos B con cajas: si B cruza A de lado a lado, se recorta la parte cubierta; si no, A queda igual. */
function subtract(a: Bounds, b: Bounds): Bounds | null {
  const cut = intersect(a, b);
  if (!cut) return a;
  const coversY = cut[0][1] <= a[0][1] && cut[1][1] >= a[1][1];
  const coversX = cut[0][0] <= a[0][0] && cut[1][0] >= a[1][0];
  if (coversX && coversY) return null;
  if (coversY) return cut[0][0] <= a[0][0] ? [[cut[1][0], a[0][1]], a[1]] : cut[1][0] >= a[1][0] ? [a[0], [cut[0][0], a[1][1]]] : a;
  if (coversX) return cut[0][1] <= a[0][1] ? [[a[0][0], cut[1][1]], a[1]] : cut[1][1] >= a[1][1] ? [a[0], [a[1][0], cut[0][1]]] : a;
  return a;
}

/**
 * Resultado plausible sin el motor: trabaja con las cajas envolventes. Unión, soldar y xor dan la
 * caja unida; la intersección, la caja común; la diferencia, A (recortada si B la cruza de lado a
 * lado); el offset, cada caja crecida (con esquinas redondas si se pidió).
 */
function mockGeometryOp(args: GeometryOpArgs): DesignObject[] {
  // La limpieza de trazos sí admite formas abiertas: no pasa por la validación de las booleanas.
  if (args.op === 'join' || args.op === 'close' || args.op === 'dedupe') return mockCleanup(args);
  const operands = boundsOfOperands(args.objects);
  const first = operands[0];
  if (!first) throw new Error('No hay objetos sobre los que operar.');
  const one = (b: Bounds | null, id = 'geo-1') => (b ? [rectObject(id, first.layer, b)] : []);
  switch (args.op) {
    case 'weld':
    case 'union':
    case 'xor':
      if (args.op !== 'weld' && operands.length < 2) throw new Error('Las booleanas necesitan al menos dos objetos.');
      return one(unionBounds(operands.map((o) => o.bounds)));
    case 'intersection':
    case 'difference': {
      if (operands.length < 2) throw new Error('Las booleanas necesitan al menos dos objetos.');
      const rest = unionBounds(operands.slice(1).map((o) => o.bounds))!;
      return one(args.op === 'intersection' ? intersect(first.bounds, rest) : subtract(first.bounds, rest));
    }
    case 'offset': {
      const d = args.distance;
      if (d === undefined || !Number.isFinite(d)) throw new Error('Falta la distancia del offset.');
      return operands.flatMap((o) => {
        const [[x0, y0], [x1, y1]] = o.bounds;
        const grown: Bounds = [[x0 - d, y0 - d], [x1 + d, y1 + d]];
        if (grown[1][0] - grown[0][0] <= 0 || grown[1][1] - grown[0][1] <= 0) return [];
        const round = (args.join ?? 'round') === 'round' && d > 0;
        return [{ ...rectObject(`${o.id}-off-1`, o.layer, grown, round ? d : 0) }];
      });
    }
    default:
      return mockCleanup(args);
  }
}

// --- Unir, cerrar y quitar duplicados -------------------------------------------------------------
// Misma regla de resultado que el motor (ver `edit/geometry.rs`): vuelve la lista completa de objetos;
// los que no cambian se devuelven idénticos y los que cambian como objetos nuevos `{op}-N`. Las
// versiones de abajo son cuadráticas (basta para el mock) pero siguen las mismas reglas.

type Poly = { points: Point[]; closed: boolean };

const CLEANUP_DEFAULT_MM = { join: 0.05, close: 0.5, dedupe: 0.01 } as const;
const dist = (a: Point, b: Point) => Math.hypot(a[0] - b[0], a[1] - b[1]);

/** Une extremos a `tol` o menos (invirtiendo si hace falta); cada resultado lleva los índices de sus fuentes. */
function joinPaths(paths: Poly[], tol: number): { path: Poly; sources: number[] }[] {
  const joinable = (p: Poly) => !p.closed && p.points.length >= 2;
  const used = paths.map(() => false);
  const out: { path: Poly; sources: number[] }[] = [];
  paths.forEach((p, seed) => {
    if (!joinable(p)) return void out.push({ path: p, sources: [seed] });
    if (used[seed]) return;
    used[seed] = true;
    let chain = [...p.points];
    const sources = [seed];
    for (;;) {
      let best: { d: number; k: number; atTail: boolean; isEnd: boolean } | null = null;
      paths.forEach((q, k) => {
        if (used[k] || !joinable(q)) return;
        for (const atTail of [true, false]) {
          for (const isEnd of [false, true]) {
            const d = dist(atTail ? chain[chain.length - 1]! : chain[0]!, isEnd ? q.points[q.points.length - 1]! : q.points[0]!);
            if (d <= tol && (!best || d < best.d)) best = { d, k, atTail, isEnd };
          }
        }
      });
      if (!best) break;
      const { k, atTail, isEnd } = best as { k: number; atTail: boolean; isEnd: boolean };
      used[k] = true;
      sources.push(k);
      const outward = (isEnd ? [...paths[k]!.points].reverse() : paths[k]!.points).slice(1);
      chain = atTail ? [...chain, ...outward] : [...outward.reverse(), ...chain];
    }
    const closed = chain.length >= 4 && dist(chain[0]!, chain[chain.length - 1]!) <= Math.max(tol, 1e-9);
    out.push({ path: { points: closed ? chain.slice(0, -1) : chain, closed }, sources });
  });
  return out;
}

/** Cierra las abiertas de 3 o más puntos cuyos extremos distan `tol` o menos. */
function closePaths(paths: Poly[], tol: number): Poly[] {
  return paths.map((p) => {
    const n = p.points.length;
    if (p.closed || n < 3 || dist(p.points[0]!, p.points[n - 1]!) > Math.max(tol, 1e-9)) return p;
    return { points: dist(p.points[0]!, p.points[n - 1]!) <= 1e-9 && n > 3 ? p.points.slice(0, -1) : p.points, closed: true };
  });
}

/** Recorta de cada segmento lo que ya cubre uno anterior (colineal dentro de `tol`); conserva el primero. */
function dedupePaths(paths: Poly[], tol: number): { path: Poly; origin: number }[] {
  type Seg = { a: Point; b: Point; len: number; path: number };
  const segs: Seg[] = [];
  const range: [number, number][] = [];
  paths.forEach((p, path) => {
    const start = segs.length;
    const count = p.closed ? p.points.length : p.points.length - 1;
    for (let k = 0; k < count; k++) {
      const a = p.points[k]!;
      const b = p.points[(k + 1) % p.points.length]!;
      segs.push({ a, b, len: dist(a, b), path });
    }
    range.push([start, segs.length]);
  });
  const lineDist = (p: Point, s: Seg) => Math.abs((p[0] - s.a[0]) * (s.b[1] - s.a[1]) - (p[1] - s.a[1]) * (s.b[0] - s.a[0])) / s.len;
  const param = (p: Point, s: Seg) => ((p[0] - s.a[0]) * (s.b[0] - s.a[0]) + (p[1] - s.a[1]) * (s.b[1] - s.a[1])) / (s.len * s.len);
  const changed = paths.map(() => false);
  const kept = segs.map((sj, j): [number, number][] => {
    if (sj.len <= 1e-9) return [[0, 1]];
    const cover: [number, number][] = [];
    for (let i = 0; i < j; i++) {
      const si = segs[i]!;
      if (si.len <= 1e-9) continue;
      const [long, short] = si.len >= sj.len ? [si, sj] : [sj, si];
      if (lineDist(short.a, long) > tol || lineDist(short.b, long) > tol) continue;
      const [t0, t1] = [param(si.a, sj), param(si.b, sj)];
      const lo = Math.max(0, Math.min(t0, t1));
      const hi = Math.min(1, Math.max(t0, t1));
      const overlap = (hi - lo) * sj.len;
      if (overlap > tol || (overlap > 1e-9 && overlap >= sj.len - 1e-9)) cover.push([lo, hi]);
    }
    if (cover.length === 0) return [[0, 1]];
    cover.sort((p, q) => p[0] - q[0]);
    const pieces: [number, number][] = [];
    let cursor = 0;
    for (const [lo, hi] of cover) {
      if (lo > cursor) pieces.push([cursor, lo]);
      cursor = Math.max(cursor, hi);
    }
    if (cursor < 1) pieces.push([cursor, 1]);
    const real = pieces.filter(([a, b]) => (b - a) * sj.len > tol);
    if (real.reduce((n, [a, b]) => n + (b - a) * sj.len, 0) < sj.len - 1e-9) changed[sj.path] = true;
    return real;
  });
  const out: { path: Poly; origin: number }[] = [];
  paths.forEach((p, idx) => {
    if (!changed[idx]) return void out.push({ path: p, origin: idx });
    const chains: Point[][] = [];
    let cur: Point[] = [];
    for (let k = range[idx]![0]; k < range[idx]![1]; k++) {
      const s = segs[k]!;
      const at = (t: number): Point => (t <= 0 ? s.a : t >= 1 ? s.b : [s.a[0] + (s.b[0] - s.a[0]) * t, s.a[1] + (s.b[1] - s.a[1]) * t]);
      for (const [t0, t1] of kept[k]!) {
        const [p0, p1] = [at(t0), at(t1)];
        if (cur.length > 0 && dist(cur[cur.length - 1]!, p0) < 1e-12) cur.push(p1);
        else {
          if (cur.length >= 2) chains.push(cur);
          cur = [p0, p1];
        }
      }
    }
    if (cur.length >= 2) chains.push(cur);
    if (p.closed && chains.length >= 2 && dist(chains[chains.length - 1]!.at(-1)!, chains[0]![0]!) < 1e-12) {
      const first = chains.shift()!;
      chains[chains.length - 1] = [...chains[chains.length - 1]!, ...first.slice(1)];
    }
    for (const points of chains) out.push({ path: { points, closed: false }, origin: idx });
  });
  return out;
}

function mockCleanup(args: GeometryOpArgs): DesignObject[] {
  const { op, objects } = args;
  if (objects.length === 0) throw new Error('No hay objetos sobre los que operar.');
  const tol = args.tolerance ?? CLEANUP_DEFAULT_MM[op as keyof typeof CLEANUP_DEFAULT_MM];
  if (!Number.isFinite(tol) || tol < 0) throw new Error('La tolerancia no es un número válido (debe ser 0 o más).');
  const unsupported = objects.filter((o) => o.kind.type !== 'vector').map((o) => o.id);
  if (unsupported.length) {
    throw new Error(`Solo se pueden limpiar los trazos de objetos vectoriales; quita las imágenes y los textos: ${list(unsupported)}.`);
  }
  const flat = objects.map((o) => objectPolylines(o));
  const taken = new Set(objects.map((o) => o.id));
  let counter = 0;
  const freshId = () => {
    let id: string;
    do id = `${op}-${++counter}`;
    while (taken.has(id));
    return id;
  };
  const make = (members: number[], paths: Poly[]): DesignObject => {
    const first = objects[members[0]!]!;
    const sameGroup = members.every((m) => objects[m]!.group === first.group);
    return {
      id: freshId(),
      layer: first.layer,
      transform: [1, 0, 0, 1, 0, 0],
      kind: { type: 'vector', paths: paths.map((p) => polylineBez(p.points, p.closed)) },
      locked: false,
      hidden: first.hidden,
      group: sameGroup ? first.group : null,
    };
  };
  const slots: (DesignObject | null | 'keep')[] = objects.map(() => 'keep');

  for (const layer of new Set(objects.map((o) => o.layer))) {
    const members = objects.flatMap((o, i) => (o.layer === layer ? [i] : []));
    const all: Poly[] = [];
    const owner: number[] = [];
    for (const m of members) for (const p of flat[m]!) {
        all.push(p);
        owner.push(m);
      }
    if (op === 'close') {
      for (const m of members) {
        const closed = closePaths(flat[m]!, tol);
        if (closed.some((p, k) => p !== flat[m]![k])) slots[m] = make([m], closed);
      }
    } else if (op === 'join') {
      const joined = joinPaths(all, tol);
      const parent = objects.map((_, i) => i);
      const find = (i: number): number => (parent[i] === i ? i : (parent[i] = find(parent[i]!)));
      const dirty = new Set<number>();
      for (const { path, sources } of joined) {
        for (const s of sources.slice(1)) {
          const [ra, rb] = [find(owner[sources[0]!]!), find(owner[s]!)];
          parent[Math.max(ra, rb)] = Math.min(ra, rb);
        }
        if (sources.length > 1 || path.closed !== all[sources[0]!]!.closed) dirty.add(owner[sources[0]!]!);
      }
      for (const root of new Set([...dirty].map(find)).values()) {
        const group = members.filter((m) => find(m) === root);
        const paths = joined.filter(({ sources }) => find(owner[sources[0]!]!) === root).map(({ path }) => path);
        group.slice(1).forEach((m) => (slots[m] = null));
        slots[group[0]!] = make(group, paths);
      }
    } else {
      const result = dedupePaths(all, tol);
      for (const m of members) {
        const own = result.filter(({ origin }) => owner[origin] === m).map(({ path }) => path);
        const original = flat[m]!;
        const same = own.length === original.length && own.every((p, k) => p === original[k]);
        if (!same) slots[m] = own.length === 0 ? null : make([m], own);
      }
    }
  }
  return objects.flatMap((o, i) => (slots[i] === 'keep' ? [structuredClone(o)] : slots[i] ? [slots[i]] : []));
}

/** Métodos simulados del área `geometry`. Trabajan con cajas envolventes para poder desarrollar la interfaz sin el motor. */
export function mockGeometry(lag: Lag): Pick<LaserApi, 'geometryOp'> {
  return {
    geometryOp: (args) => lag(() => mockGeometryOp(args)),
  };
}
