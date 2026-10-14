import { polylineBez } from '@/lib/bezpath';
import { concaveHull } from '@/lib/concaveHull';
import { objectPolylines, pointsBounds, type Bounds } from '@/lib/geometry';
import { convexHull } from '@/lib/hull';
import { makeWarp } from '@/lib/warp';
import type { LaserApi } from '../api';
import type { DesignObject, HullArgs, Point, TangentArgs, TangentCircle, WarpArgs } from '../types';

type Lag = <T>(value: () => T) => Promise<T>;

const list = (objects: DesignObject[]) => objects.map((o) => `«${o.id}»`).join(', ');

/** Divide cada tramo en trozos de a lo sumo `step` mm: una recta deformada deja de ser recta. */
function densify(points: Point[], closed: boolean, step: number): Point[] {
  const out: Point[] = [];
  const n = points.length;
  const last = closed ? n : n - 1;
  for (let i = 0; i < last; i++) {
    const a = points[i]!;
    const b = points[(i + 1) % n]!;
    const parts = Math.max(1, Math.ceil(Math.hypot(b[0] - a[0], b[1] - a[1]) / step));
    for (let k = 0; k < parts; k++) out.push([a[0] + ((b[0] - a[0]) * k) / parts, a[1] + ((b[1] - a[1]) * k) / parts]);
  }
  if (!closed && n > 0) out.push(points[n - 1]!);
  return out;
}

/**
 * Deformación simulada: aplana, subdivide y mueve los puntos por la misma envolvente que el motor;
 * el resultado son polilíneas (el motor real devuelve cúbicas ajustadas).
 */
function mockWarp(args: WarpArgs): DesignObject[] {
  if (args.objects.length === 0) throw new Error('No hay objetos que deformar.');
  const images = args.objects.filter((o) => o.kind.type === 'image');
  if (images.length) throw new Error(`Las imágenes no se pueden deformar: ${list(images)}.`);
  const warp = makeWarp(args.bounds as Bounds, args.envelope);
  if (!warp) {
    const flat = !(args.bounds[1][0] - args.bounds[0][0] > 1e-9) || !(args.bounds[1][1] - args.bounds[0][1] > 1e-9);
    throw new Error(flat ? 'La selección no tiene ancho y alto: no se puede deformar.' : 'Las esquinas de la perspectiva no forman un cuadrilátero convexo.');
  }
  const step = Math.max(args.bounds[1][0] - args.bounds[0][0], args.bounds[1][1] - args.bounds[0][1]) / 80;
  return args.objects.map((o) => {
    const polys = objectPolylines(o, 0.05);
    if (polys.length === 0) throw new Error(`El objeto «${o.id}» no tiene contorno que deformar (¿un texto sin contorno generado?).`);
    const paths = polys.map((p) => polylineBez(densify(p.points, p.closed, Math.max(step, 0.2)).map(warp), p.closed));
    return { ...o, transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths }, shape: null, start: null, manual_tabs: [] };
  });
}

function mockHull(args: HullArgs): DesignObject[] {
  const first = args.objects[0];
  if (!first) throw new Error('No hay objetos de los que sacar el contorno.');
  const points = args.objects.flatMap((o) => objectPolylines(o)).flatMap((p) => p.points);
  const edge = args.max_edge;
  if (edge !== undefined && edge !== null && !(edge > 0)) throw new Error('La distancia máxima de arista debe ser un número positivo.');
  const outline = edge === undefined || edge === null ? convexHull(points) : concaveHull(points, edge);
  if (outline.length < 3) throw new Error('La selección no tiene área: no se puede envolver.');
  return [
    {
      id: 'hull-1',
      layer: args.layer ?? first.layer,
      transform: [1, 0, 0, 1, 0, 0],
      kind: { type: 'vector', paths: [polylineBez(outline, true)] },
      locked: false,
      hidden: false,
      group: null,
    },
  ];
}

interface Disc {
  c: Point;
  rho: number;
}

/** Cada operando se aproxima por el círculo de su caja (una recta, por un punto): basta para el simulador. */
function discOf(o: DesignObject): Disc {
  const box = pointsBounds(objectPolylines(o).flatMap((p) => p.points));
  if (!box) throw new Error('Alguna de las formas no tiene trazo del que ser tangente.');
  const w = box[1][0] - box[0][0];
  const h = box[1][1] - box[0][1];
  const closed = objectPolylines(o).some((p) => p.closed);
  return { c: [(box[0][0] + box[1][0]) / 2, (box[0][1] + box[1][1]) / 2], rho: closed ? Math.max(w, h) / 2 : 0 };
}

function mockTangent(args: TangentArgs): TangentCircle[] {
  const n = args.objects.length;
  if (n < 2 || n > 3) throw new Error('Selecciona dos o tres formas o líneas para el círculo tangente.');
  const discs = args.objects.map(discOf);
  if (n === 2) {
    const r = args.radius;
    if (r === undefined || r === null) throw new Error('Falta el radio del círculo tangente.');
    if (!(r > 0)) throw new Error('El radio debe ser un número positivo.');
    const [a, b] = discs as [Disc, Disc];
    const ra = a.rho + r;
    const rb = b.rho + r;
    const d = Math.hypot(b.c[0] - a.c[0], b.c[1] - a.c[1]);
    if (d < 1e-9 || d > ra + rb || d < Math.abs(ra - rb)) return [];
    const x = (d * d + ra * ra - rb * rb) / (2 * d);
    const h = Math.sqrt(Math.max(0, ra * ra - x * x));
    const ux = (b.c[0] - a.c[0]) / d;
    const uy = (b.c[1] - a.c[1]) / d;
    const mid: Point = [a.c[0] + ux * x, a.c[1] + uy * x];
    return ([1, -1] as const)
      .map((s): TangentCircle => ({ center: [mid[0] - uy * h * s, mid[1] + ux * h * s], radius: r }))
      .filter((c, i, all) => i === 0 || Math.hypot(c.center[0] - all[0]!.center[0], c.center[1] - all[0]!.center[1]) > 1e-9);
  }
  // Tres: tangente exterior (Apolonio). Restando la primera ecuación a las otras dos queda un
  // sistema lineal en x, y con r como parámetro; al sustituir en la primera sale una cuadrática en r.
  const [p, q, s] = discs as [Disc, Disc, Disc];
  const lin = (o: Disc) => ({
    a: 2 * (o.c[0] - p.c[0]),
    b: 2 * (o.c[1] - p.c[1]),
    c: 2 * (o.rho - p.rho),
    d: o.c[0] ** 2 + o.c[1] ** 2 - o.rho ** 2 - (p.c[0] ** 2 + p.c[1] ** 2 - p.rho ** 2),
  });
  const e1 = lin(q);
  const e2 = lin(s);
  const det = e1.a * e2.b - e2.a * e1.b;
  if (Math.abs(det) < 1e-9) return [];
  // x = (d·b' - ...) / det con r: x = X0 + X1·r, y = Y0 + Y1·r
  const X0 = (e1.d * e2.b - e2.d * e1.b) / det;
  const X1 = (-e1.c * e2.b + e2.c * e1.b) / det;
  const Y0 = (e1.a * e2.d - e2.a * e1.d) / det;
  const Y1 = (-e1.a * e2.c + e2.a * e1.c) / det;
  const dx = X0 - p.c[0];
  const dy = Y0 - p.c[1];
  const A = X1 * X1 + Y1 * Y1 - 1;
  const B = 2 * (dx * X1 + dy * Y1 - p.rho);
  const C = dx * dx + dy * dy - p.rho * p.rho;
  const roots = Math.abs(A) < 1e-12 ? [-C / B] : [(-B + Math.sqrt(B * B - 4 * A * C)) / (2 * A), (-B - Math.sqrt(B * B - 4 * A * C)) / (2 * A)];
  return roots
    .filter((r) => Number.isFinite(r) && r > 1e-9)
    .map((r): TangentCircle => ({ center: [X0 + X1 * r, Y0 + Y1 * r], radius: r }))
    .sort((u, v) => u.radius - v.radius);
}

/** Deformar, contorno envolvente y círculo tangente simulados (ver `edit/design.rs` para el real). */
export function mockDesign(lag: Lag): Pick<LaserApi, 'warpObjects' | 'hullOutline' | 'tangentCircles'> {
  return {
    warpObjects: (args) => lag(() => mockWarp(args)),
    hullOutline: (args) => lag(() => mockHull(args)),
    tangentCircles: (args) => lag(() => mockTangent(args)),
  };
}
