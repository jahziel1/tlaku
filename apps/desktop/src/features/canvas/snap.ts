import type { DesignObject, Point } from '@/api/types';
import type { Bounds } from '@/lib/geometry';
import { getGeom, type Box } from './geom';

/**
 * Ajuste magnético predecible:
 *  - La tolerancia se da en píxeles de pantalla y se convierte a mm con el zoom actual, así que
 *    "cerca" se ve igual a cualquier zoom.
 *  - Cada eje se resuelve por separado: gana el candidato más cercano; en un empate, un objeto
 *    o la cama ganan a la cuadrícula.
 *  - La cuadrícula es la que se ve (paso menor visible), no una oculta.
 */
export const SNAP_PX = 8;

export type SnapKind = 'object' | 'bed' | 'grid';

/** Línea de referencia en un eje. `lo..hi` es su extensión en el otro eje. */
export interface SnapTarget {
  v: number;
  kind: Exclude<SnapKind, 'grid'>;
  lo: number;
  hi: number;
}

export interface SnapSet {
  xs: SnapTarget[];
  ys: SnapTarget[];
}

export interface SnapOptions {
  /** Tolerancia en mm (`SNAP_PX / zoom`). */
  tol: number;
  /** Paso de cuadrícula en mm o `null` si el ajuste a cuadrícula está apagado. */
  grid: number | null;
  /** Ajuste a objetos y cama. */
  objects: boolean;
}

/** Línea guía que se dibuja en el lienzo. */
export interface Guide {
  axis: 'x' | 'y';
  v: number;
  lo: number;
  hi: number;
  kind: SnapKind;
}

/** Característica móvil de lo que se arrastra (un borde, el centro, un punto). */
export interface Feature {
  v: number;
  /** ¿Puede pegarse a la cuadrícula? (los centros no). */
  grid: boolean;
}

export function snapToleranceMM(zoom: number, px = SNAP_PX): number {
  return px / zoom;
}

/** Líneas de referencia: bordes y centros de los demás objetos y de la cama. */
export function collectSnapTargets(
  objects: readonly DesignObject[],
  skip: ReadonlySet<string>,
  bed: { width: number; height: number },
  region?: Box,
): SnapSet {
  const xs: SnapTarget[] = [];
  const ys: SnapTarget[] = [];
  for (const obj of objects) {
    if (obj.hidden || skip.has(obj.id)) continue;
    const box = getGeom(obj).box;
    if (!box) continue;
    if (region && (box[2] < region[0] || box[0] > region[2] || box[3] < region[1] || box[1] > region[3])) continue;
    for (const v of [box[0], (box[0] + box[2]) / 2, box[2]]) xs.push({ v, kind: 'object', lo: box[1], hi: box[3] });
    for (const v of [box[1], (box[1] + box[3]) / 2, box[3]]) ys.push({ v, kind: 'object', lo: box[0], hi: box[2] });
  }
  for (const v of [0, bed.width / 2, bed.width]) xs.push({ v, kind: 'bed', lo: 0, hi: bed.height });
  for (const v of [0, bed.height / 2, bed.height]) ys.push({ v, kind: 'bed', lo: 0, hi: bed.width });
  return { xs, ys };
}

interface Pick {
  delta: number;
  kind: SnapKind;
}

function pickAxis(features: readonly Feature[], targets: readonly SnapTarget[], opts: SnapOptions): Pick | null {
  let best: Pick | null = null;
  let bestDist = opts.tol + 1e-9;
  const consider = (delta: number, kind: SnapKind) => {
    const d = Math.abs(delta);
    // Un objeto o la cama ganan a la cuadrícula si empatan (hasta 1e-9 mm).
    if (d < bestDist - 1e-9 || (d <= bestDist + 1e-9 && best && best.kind === 'grid' && kind !== 'grid')) {
      best = { delta, kind };
      bestDist = d;
    }
  };
  if (opts.objects) {
    for (const f of features) for (const t of targets) consider(t.v - f.v, t.kind);
  }
  if (opts.grid && opts.grid > 0) {
    for (const f of features) {
      if (f.grid) consider(Math.round(f.v / opts.grid) * opts.grid - f.v, 'grid');
    }
  }
  return best;
}

export interface SnapResult {
  dx: number;
  dy: number;
  guides: Guide[];
  /** Hubo ajuste en algún eje. */
  snapped: boolean;
}

export const NO_SNAP: SnapResult = { dx: 0, dy: 0, guides: [], snapped: false };

/**
 * Calcula cuánto hay que desplazar las características `fx`/`fy` para que se peguen a los
 * candidatos. `spanX` = extensión en Y de lo que se mueve (para dibujar las guías verticales) y
 * `spanY` = extensión en X.
 */
export function snapFeatures(
  fx: readonly Feature[],
  fy: readonly Feature[],
  spanX: [number, number],
  spanY: [number, number],
  set: SnapSet,
  opts: SnapOptions,
): SnapResult {
  const px = fx.length ? pickAxis(fx, set.xs, opts) : null;
  const py = fy.length ? pickAxis(fy, set.ys, opts) : null;
  const dx = px?.delta ?? 0;
  const dy = py?.delta ?? 0;
  const guides: Guide[] = [];
  const seen = new Set<string>();
  const push = (g: Guide) => {
    const key = `${g.axis}:${Math.round(g.v * 1e6)}`;
    if (!seen.has(key)) {
      seen.add(key);
      guides.push(g);
    } else {
      const old = guides.find((o) => o.axis === g.axis && Math.abs(o.v - g.v) < 1e-6);
      if (old) {
        old.lo = Math.min(old.lo, g.lo);
        old.hi = Math.max(old.hi, g.hi);
        if (g.kind !== 'grid') old.kind = g.kind;
      }
    }
  };
  const gather = (axis: 'x' | 'y', feats: readonly Feature[], delta: number, picked: Pick | null, targets: readonly SnapTarget[], span: [number, number], otherDelta: number) => {
    if (!picked) return;
    const lo = span[0] + otherDelta;
    const hi = span[1] + otherDelta;
    for (const f of feats) {
      const moved = f.v + delta;
      if (opts.objects) {
        for (const t of targets) {
          if (Math.abs(t.v - moved) < 1e-6) push({ axis, v: t.v, lo: Math.min(lo, t.lo), hi: Math.max(hi, t.hi), kind: t.kind });
        }
      }
      // La cuadrícula ya se ve: solo se marca cuando el ajuste realmente movió algo en ese eje.
      if (picked.kind === 'grid' && Math.abs(delta) > 1e-9 && opts.grid && f.grid && Math.abs(Math.round(moved / opts.grid) * opts.grid - moved) < 1e-6) {
        push({ axis, v: moved, lo, hi, kind: 'grid' });
      }
    }
  };
  gather('x', fx, dx, px, set.xs, spanX, dy);
  gather('y', fy, dy, py, set.ys, spanY, dx);
  return { dx, dy, guides, snapped: px !== null || py !== null };
}

/** Ajuste de una caja que se mueve entera (bordes y centros; la cuadrícula solo a los bordes). */
export function snapMove(b: Bounds, set: SnapSet, opts: SnapOptions, axes: { x: boolean; y: boolean } = { x: true, y: true }): SnapResult {
  const fx: Feature[] = axes.x
    ? [{ v: b[0][0], grid: true }, { v: (b[0][0] + b[1][0]) / 2, grid: false }, { v: b[1][0], grid: true }]
    : [];
  const fy: Feature[] = axes.y
    ? [{ v: b[0][1], grid: true }, { v: (b[0][1] + b[1][1]) / 2, grid: false }, { v: b[1][1], grid: true }]
    : [];
  return snapFeatures(fx, fy, [b[0][1], b[1][1]], [b[0][0], b[1][0]], set, opts);
}

/** Ajuste de un punto (cursor al dibujar, manija al escalar). `ex`/`ey` desactivan un eje. */
export function snapPoint(p: Point, set: SnapSet, opts: SnapOptions, axes: { x: boolean; y: boolean } = { x: true, y: true }): SnapResult {
  return snapFeatures(
    axes.x ? [{ v: p[0], grid: true }] : [],
    axes.y ? [{ v: p[1], grid: true }] : [],
    [p[1], p[1]],
    [p[0], p[0]],
    set,
    opts,
  );
}
