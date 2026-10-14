import type { DesignObject, Point, WarpEnvelope, WarpGrid } from '@/api/types';
import { polylineBez, type BezPath } from '@/lib/bezpath';
import { objectPolylines, type Bounds } from '@/lib/geometry';
import { homography, identityEnvelope, identityGrid, makeWarp } from '@/lib/warp';
import { envelopeOf, type WarpParams } from '@/features/arrange/warpPresets';

/**
 * Lógica pura del modo de edición de Deformar en el lienzo (sin DOM ni store): la malla de control,
 * qué manija hay bajo el puntero, cómo se mueve un punto, cómo se pasa de un modo a otro y la vista
 * previa viva. La deformación en sí es la misma de `lib/warp.ts` (espejo de `tlaku_geom::Warp`); el
 * motor la repite con curvas exactas al aplicar.
 *
 *  - `bilinear`: cuatro esquinas, bordes rectos (parche de Coons con bordes rectos).
 *  - `perspective`: cuatro esquinas con proyección (las rectas siguen rectas).
 *  - `mesh`: malla de 4x4 puntos, 16 manijas; la superficie pasa por cada una.
 */

export type WarpMode = 'bilinear' | 'perspective' | 'mesh';
export const WARP_MODES: readonly WarpMode[] = ['bilinear', 'perspective', 'mesh'];

type Corners = [Point, Point, Point, Point];

export interface WarpSession {
  mode: WarpMode;
  /** Región de origen que se deforma (la caja de la selección). */
  bounds: Bounds;
  /** Esquinas de destino: abajo-izquierda, abajo-derecha, arriba-derecha, arriba-izquierda. */
  corners: Corners;
  /** Puntos de la malla, de abajo hacia arriba y de izquierda a derecha. */
  grid: WarpGrid;
}

/** Esquinas de una caja en el orden de la sesión. */
export function boxCorners(b: Bounds): Corners {
  const [[x0, y0], [x1, y1]] = b;
  return [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
}

export function newSession(bounds: Bounds, mode: WarpMode = 'bilinear'): WarpSession {
  return { mode, bounds, corners: boxCorners(bounds), grid: identityGrid(bounds) };
}

/** Cuántas manijas tiene el modo. */
export const handleCount = (mode: WarpMode): number => (mode === 'mesh' ? 16 : 4);

/** Posición (mm) de cada manija. En la malla, `j * 4 + i` (fila `j` de abajo hacia arriba). */
export function handlePoints(s: WarpSession): Point[] {
  return s.mode === 'mesh' ? s.grid.flat() : [...s.corners];
}

/** Envolvente que se manda al motor. */
export function envelopeOfSession(s: WarpSession): WarpEnvelope {
  const base = boxCorners(s.bounds);
  if (s.mode === 'mesh') return { ...identityEnvelope(s.bounds), grid: s.grid };
  if (s.mode === 'perspective') return { ...identityEnvelope(s.bounds), perspective: s.corners };
  const params: WarpParams = {
    corners: s.corners.map((c, i) => [c[0] - base[i]![0], c[1] - base[i]![1]] as Point) as WarpParams['corners'],
    bulge: [0, 0, 0, 0],
  };
  return envelopeOf(params, s.bounds);
}

/** Función de deformación de la sesión, o `null` si no está definida (proyección con un cuadrilátero cóncavo). */
export function warpOf(s: WarpSession): ((p: Point) => Point) | null {
  return makeWarp(s.bounds, envelopeOfSession(s));
}

/** ¿La sesión deja el diseño donde está? */
export function isIdentity(s: WarpSession): boolean {
  const same = (a: readonly Point[], b: readonly Point[]) => a.every((p, i) => Math.abs(p[0] - b[i]![0]) < 1e-9 && Math.abs(p[1] - b[i]![1]) < 1e-9);
  return s.mode === 'mesh' ? same(s.grid.flat(), identityGrid(s.bounds).flat()) : same(s.corners, boxCorners(s.bounds));
}

/** ¿Se puede aplicar? (la proyección necesita un cuadrilátero convexo). */
export function isValid(s: WarpSession): boolean {
  return s.mode !== 'perspective' || homography(s.corners) !== null;
}

/** Índice de la manija más cercana a `p` a menos de `tol` mm, o -1. Gana la primera en un empate. */
export function hitHandle(s: WarpSession, p: Point, tol: number): number {
  let best = -1;
  let bestD = tol;
  handlePoints(s).forEach((h, i) => {
    const d = Math.hypot(h[0] - p[0], h[1] - p[1]);
    if (d <= bestD && (best < 0 || d < bestD)) {
      best = i;
      bestD = d;
    }
  });
  return best;
}

/**
 * Mueve la manija `index` a `to`. Devuelve la misma sesión si el índice no existe, el punto no es
 * finito o (con perspectiva) el resultado dejaría de ser un cuadrilátero convexo.
 */
export function movePoint(s: WarpSession, index: number, to: Point): WarpSession {
  if (!Number.isFinite(to[0]) || !Number.isFinite(to[1]) || index < 0 || index >= handleCount(s.mode)) return s;
  if (s.mode === 'mesh') {
    const j = Math.floor(index / 4);
    const i = index % 4;
    const grid = s.grid.map((row, r) => (r === j ? row.map((p, c) => (c === i ? to : p)) : row)) as WarpGrid;
    return { ...s, grid };
  }
  const corners = s.corners.map((c, i) => (i === index ? to : c)) as Corners;
  const next = { ...s, corners };
  return isValid(next) ? next : s;
}

/** Cambia de modo conservando la forma: de esquinas a malla se muestrea la deformación; de malla a esquinas se toman sus cuatro vértices. */
export function setMode(s: WarpSession, mode: WarpMode): WarpSession {
  if (mode === s.mode) return s;
  if (mode === 'mesh') {
    const warp = warpOf(s);
    if (!warp) return { ...s, mode };
    const [[x0, y0], [x1, y1]] = s.bounds;
    const row = (j: number) => [0, 1, 2, 3].map((i) => warp([x0 + ((x1 - x0) * i) / 3, y0 + ((y1 - y0) * j) / 3])) as WarpGrid[number];
    return { ...s, mode, grid: [row(0), row(1), row(2), row(3)] };
  }
  if (s.mode === 'mesh') {
    const g = s.grid;
    const corners: Corners = [g[0][0], g[0][3], g[3][3], g[3][0]];
    const next = { ...s, mode, corners };
    // Una malla muy retorcida puede dar cuatro esquinas sin proyección: se vuelve a la caja.
    return isValid(next) ? next : { ...next, corners: boxCorners(s.bounds) };
  }
  const next = { ...s, mode };
  return isValid(next) ? next : { ...next, corners: boxCorners(s.bounds) };
}

/** Vuelve la sesión a la identidad (mismo modo). */
export function resetSession(s: WarpSession): WarpSession {
  return newSession(s.bounds, s.mode);
}

// ---- vista previa viva -------------------------------------------------------------------------------------

/** Contornos del diseño ya aplanados y subdivididos, listos para deformar en cada cuadro del arrastre. */
export interface PreviewSource {
  polys: { points: Point[]; closed: boolean }[];
  /** Cantidad total de puntos (para acotar el costo de cada cuadro). */
  count: number;
}

/** Máximo de puntos que se deforman en cada cuadro del arrastre. */
export const PREVIEW_MAX_POINTS = 24_000;
const FLATTEN_MM = 0.05;

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
 * Prepara la vista previa: aplana los objetos y subdivide los tramos largos (una recta deformada deja
 * de ser recta). Se hace una sola vez al activar la herramienta; cada cuadro solo mueve puntos.
 */
export function previewSource(objects: readonly DesignObject[], bounds: Bounds): PreviewSource {
  const span = Math.max(bounds[1][0] - bounds[0][0], bounds[1][1] - bounds[0][1]);
  const flat = objects.flatMap((o) => objectPolylines(o, FLATTEN_MM));
  let step = Math.max(span / 80, 0.2);
  for (let tries = 0; tries < 6; tries++) {
    const polys = flat.map((p) => ({ points: densify(p.points, p.closed, step), closed: p.closed }));
    const count = polys.reduce((n, p) => n + p.points.length, 0);
    if (count <= PREVIEW_MAX_POINTS || tries === 5) return { polys, count };
    step *= 2;
  }
  return { polys: [], count: 0 };
}

/** Contornos deformados por `warp`, como trayectorias de polilínea. */
export function previewPaths(src: PreviewSource, warp: (p: Point) => Point): BezPath[] {
  return src.polys.map((p) => polylineBez(p.points.map(warp), p.closed));
}

const LATTICE_STEPS = 24;

/** Líneas guía de la malla de control deformada: contorno y, en la malla, las líneas interiores. */
export function guidePaths(s: WarpSession, warp: (p: Point) => Point): BezPath[] {
  const [[x0, y0], [x1, y1]] = s.bounds;
  const line = (f: (t: number) => Point): BezPath => polylineBez(Array.from({ length: LATTICE_STEPS + 1 }, (_, k) => warp(f(k / LATTICE_STEPS))), false);
  const cuts = s.mode === 'mesh' ? [0, 1 / 3, 2 / 3, 1] : [0, 1];
  const out: BezPath[] = [];
  for (const c of cuts) {
    out.push(line((t) => [x0 + (x1 - x0) * t, y0 + (y1 - y0) * c]));
    out.push(line((t) => [x0 + (x1 - x0) * c, y0 + (y1 - y0) * t]));
  }
  return out;
}
