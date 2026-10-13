import type { Affine, DesignObject, Document, Point } from '@/api/types';
import type { I18nKey } from '@/i18n';
import { objectPolylines } from '@/lib/geometry';
import { groupIdsOf, regroupCopies } from '@/lib/groups';
import { multiplyAffine } from '@/lib/geometry';
import { uniqueId } from '@/store/document';
import { MAX_ARRAY_PIECES, rotationAbout, translationMatrix } from './arrays';
import { boundsCenter, type Unit } from './units';

/**
 * Copiar a lo largo de un trazo (D69): copias del contenido repartidas sobre un trayecto. El
 * centro de la caja del contenido es el punto de anclaje: se lleva a cada punto del trayecto y, si
 * se pide, gira para seguir la tangente (el contenido se considera orientado hacia +X).
 */

export type AlongMode = 'count' | 'spacing';

export interface AlongPathParams {
  mode: AlongMode;
  /** Copias en total (modo `count`; con un trayecto abierto, la primera va al inicio y la última al final). */
  count: number;
  /** Distancia (mm) entre copias medida sobre el trayecto (modo `spacing`). */
  spacing: number;
  /** Girar cada copia según la tangente del trayecto. */
  rotate: boolean;
  /** Giro extra (grados) sobre la tangente. */
  angle: number;
  /** Conservar el contenido original además de las copias. */
  keepOriginal: boolean;
}

export const DEFAULT_ALONG: AlongPathParams = { mode: 'count', count: 8, spacing: 10, rotate: true, angle: 0, keepOriginal: false };

export interface PathSample {
  point: Point;
  /** Dirección de la tangente, en radianes (antihoraria desde +X). */
  angle: number;
}

export interface SampledPath {
  length: number;
  closed: boolean;
  at: (s: number) => PathSample;
}

/** Polilínea del trayecto elegido: la más larga de la unidad. `null` si no tiene trazo. */
export function pathOfUnit(unit: Pick<Unit, 'objects'>): { points: Point[]; closed: boolean } | null {
  let best: { points: Point[]; closed: boolean } | null = null;
  let bestLength = 0;
  for (const o of unit.objects) {
    for (const p of objectPolylines(o)) {
      if (p.points.length < 2) continue;
      const pts = p.closed ? [...p.points, p.points[0]!] : p.points;
      let len = 0;
      for (let i = 1; i < pts.length; i++) len += Math.hypot(pts[i]![0] - pts[i - 1]![0], pts[i]![1] - pts[i - 1]![1]);
      if (len > bestLength) {
        bestLength = len;
        best = { points: p.points, closed: p.closed };
      }
    }
  }
  return best;
}

/** Evaluador por longitud de arco del trayecto (cerrado: vuelve al primer punto). `null` si no tiene largo. */
export function samplePath(points: readonly Point[], closed: boolean): SampledPath | null {
  const pts = closed && points.length > 2 ? [...points, points[0]!] : [...points];
  const starts: number[] = [0];
  for (let i = 1; i < pts.length; i++) starts.push(starts[i - 1]! + Math.hypot(pts[i]![0] - pts[i - 1]![0], pts[i]![1] - pts[i - 1]![1]));
  const length = starts[starts.length - 1] ?? 0;
  if (!(length > 1e-9)) return null;
  const at = (s: number): PathSample => {
    const d = Math.min(Math.max(s, 0), length);
    let i = 1;
    while (i < pts.length - 1 && starts[i]! < d) i++;
    const a = pts[i - 1]!;
    const b = pts[i]!;
    const seg = starts[i]! - starts[i - 1]!;
    const t = seg > 1e-12 ? (d - starts[i - 1]!) / seg : 0;
    return { point: [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t], angle: Math.atan2(b[1] - a[1], b[0] - a[0]) };
  };
  return { length, closed: closed && points.length > 2, at };
}

/** Distancias (mm, sobre el trayecto) a las que va cada copia. */
export function stationsOf(path: SampledPath, p: Pick<AlongPathParams, 'mode' | 'count' | 'spacing'>): number[] {
  if (p.mode === 'count') {
    const n = Math.round(p.count);
    if (!(n >= 1)) return [];
    if (n === 1) return [0];
    const step = path.closed ? path.length / n : path.length / (n - 1);
    return Array.from({ length: n }, (_, i) => i * step);
  }
  if (!(p.spacing > 1e-9)) return [];
  const out: number[] = [];
  const end = path.closed ? path.length - 1e-9 : path.length + 1e-9;
  for (let s = 0; s <= end; s += p.spacing) {
    out.push(s);
    if (out.length > MAX_ARRAY_PIECES + 1) break;
  }
  return out;
}

export interface AlongPlan {
  /** Una matriz por copia, en coordenadas de documento (aplicar encima del `transform` de cada objeto). */
  matrices: Affine[];
  /** Puntos de anclaje de cada copia (para la vista previa). */
  anchors: Point[];
  issue: I18nKey | null;
}

/** Copias del bloque cuyo centro es `ref` a lo largo del trayecto. */
export function planAlongPath(ref: Point, path: SampledPath | null, p: AlongPathParams): AlongPlan {
  const empty = (issue: I18nKey): AlongPlan => ({ matrices: [], anchors: [], issue });
  if (!path) return empty('design.along.err.noPath');
  if (![p.count, p.spacing, p.angle].every(Number.isFinite)) return empty('design.along.err.numbers');
  const stations = stationsOf(path, p);
  if (stations.length === 0) return empty(p.mode === 'count' ? 'design.along.err.count' : 'design.along.err.spacing');
  if (stations.length > MAX_ARRAY_PIECES) return empty('design.along.err.tooMany');
  const matrices: Affine[] = [];
  const anchors: Point[] = [];
  for (const s of stations) {
    const { point, angle } = path.at(s);
    anchors.push(point);
    // Primero se lleva el ancla al punto; después se gira sobre él.
    const move = translationMatrix(point[0] - ref[0], point[1] - ref[1]);
    const deg = p.rotate ? (angle * 180) / Math.PI + p.angle : p.angle;
    matrices.push(Math.abs(deg) < 1e-9 ? move : multiplyAffine(rotationAbout(point[0], point[1], deg), move));
  }
  return { matrices, anchors, issue: null };
}

/** Centro de la caja del contenido. */
export function contentCenter(content: readonly Unit[]): Point | null {
  const objs = content.flatMap((u) => u.objects);
  if (objs.length === 0) return null;
  const xs = content.flatMap((u) => [u.bounds[0][0], u.bounds[1][0]]);
  const ys = content.flatMap((u) => [u.bounds[0][1], u.bounds[1][1]]);
  return boundsCenter([[Math.min(...xs), Math.min(...ys)], [Math.max(...xs), Math.max(...ys)]]);
}

/**
 * Agrega una copia del contenido por matriz. Con `keepOriginal` falso, los originales se quitan.
 * Las copias llevan ids y grupos propios y van al final (encima de todo). Devuelve el documento y los ids nuevos.
 */
export function buildAlongPath(doc: Document, content: readonly Unit[], matrices: readonly Affine[], keepOriginal: boolean): { doc: Document; created: string[] } {
  const source: DesignObject[] = content.flatMap((u) => u.objects);
  const taken = new Set(doc.objects.map((o) => o.id));
  const groups = groupIdsOf(doc.objects);
  const created: string[] = [];
  const copies = matrices.flatMap((m) =>
    regroupCopies(
      source.map((o) => {
        const id = uniqueId(o.id.replace(/-\d+$/, ''), taken);
        taken.add(id);
        created.push(id);
        return { ...o, id, locked: false, transform: multiplyAffine(m, o.transform) };
      }),
      groups,
    ),
  );
  if (copies.length === 0) return { doc, created };
  const removed = keepOriginal ? new Set<string>() : new Set(source.map((o) => o.id));
  return { doc: { ...doc, objects: [...doc.objects.filter((o) => !removed.has(o.id)), ...copies] }, created };
}
