import type { DesignObject, Document, Point } from '@/api/types';
import { selectionUnits } from '@/features/arrange/units';
import { ellipseBez, polylineBez, type BezPath } from '@/lib/bezpath';
import { objectBounds, unionBounds, type Bounds } from '@/lib/geometry';
import { GUIDE_COLOR, isGuideColor } from '@/lib/palette';
import { uniqueId } from '@/store/document';

/** Una marca de registro del diseño: los objetos que la forman y su centro en mm. */
export interface DesignMark {
  ids: string[];
  point: Point;
}

export type MarkKind = 'cross' | 'circle';
export type MarkCount = 2 | 3 | 4;

export const MARK_COUNTS: readonly MarkCount[] = [2, 3, 4];
export const MIN_MARKS = 2;
export const MAX_MARKS = 4;
/** Distancia mínima entre marcas para medir bien el giro (mm). Más cerca el asistente no avanza. */
export const MIN_MARK_DISTANCE_MM = 10;
/** Medio largo del brazo de la cruz y distancia de la marca al borde del diseño (mm). */
export const MARK_ARM_MM = 5;
export const MARK_MARGIN_MM = 8;

/** Caja del diseño que se corta: lo visible que no es una guía. `null` si no hay nada. */
export function designBox(doc: Pick<Document, 'objects'>): Bounds | null {
  return unionBounds(doc.objects.filter((o) => !o.hidden && !isGuideColor(o.layer)).map(objectBounds));
}

/** Centros de las marcas en las esquinas de la caja: 2 en diagonal, 3 en L y 4 en las cuatro esquinas. */
export function cornerCenters(box: Bounds, count: MarkCount, margin = MARK_MARGIN_MM): Point[] {
  const [[x0, y0], [x1, y1]] = box;
  const bl: Point = [x0 - margin, y0 - margin];
  const br: Point = [x1 + margin, y0 - margin];
  const tr: Point = [x1 + margin, y1 + margin];
  const tl: Point = [x0 - margin, y1 + margin];
  if (count === 2) return [bl, tr];
  if (count === 3) return [bl, br, tl];
  return [bl, br, tr, tl];
}

/** Curvas de una marca centrada en `c`: cruz, o círculo con una cruz pequeña al centro. */
export function markPaths(c: Point, kind: MarkKind, arm = MARK_ARM_MM): BezPath[] {
  const [x, y] = c;
  const cross = (a: number): BezPath[] => [polylineBez([[x - a, y], [x + a, y]], false), polylineBez([[x, y - a], [x, y + a]], false)];
  return kind === 'cross' ? cross(arm) : [ellipseBez(x, y, arm, arm), ...cross(arm / 2)];
}

/**
 * Objetos de las marcas de registro, en la capa de guías (no salen a la máquina: se imprimen con
 * el diseño, no se cortan). Cada marca es un solo objeto.
 */
export function registrationMarkObjects(
  doc: Pick<Document, 'objects'>,
  box: Bounds,
  kind: MarkKind,
  count: MarkCount,
): { objects: DesignObject[]; marks: DesignMark[] } {
  const taken = new Set(doc.objects.map((o) => o.id));
  const objects: DesignObject[] = [];
  const marks: DesignMark[] = [];
  for (const point of cornerCenters(box, count)) {
    const id = uniqueId('printcut-mark', taken);
    taken.add(id);
    objects.push({
      id,
      layer: GUIDE_COLOR,
      transform: [1, 0, 0, 1, 0, 0],
      kind: { type: 'vector', paths: markPaths(point, kind) },
      locked: false,
      hidden: false,
      group: null,
    });
    marks.push({ ids: [id], point });
  }
  return { objects, marks };
}

export type SelectionMarksError = 'count' | 'close';

/** La menor distancia entre dos puntos de la lista (Infinity con menos de dos). */
export function closestPair(points: readonly Point[]): number {
  let best = Infinity;
  for (let i = 0; i < points.length; i++) {
    for (let j = i + 1; j < points.length; j++) best = Math.min(best, Math.hypot(points[i]![0] - points[j]![0], points[i]![1] - points[j]![1]));
  }
  return best;
}

/**
 * Marcas a partir de la selección: cada objeto (o grupo completo) es una marca y su centro es el
 * de su caja. Hacen falta entre 2 y 4, separadas al menos `MIN_MARK_DISTANCE_MM`.
 */
export function marksFromSelection(doc: Pick<Document, 'objects'>, selectedIds: readonly string[]): { marks: DesignMark[] } | { error: SelectionMarksError } {
  const units = selectionUnits(doc, selectedIds);
  if (units.length < MIN_MARKS || units.length > MAX_MARKS) return { error: 'count' };
  const marks = units.map((u): DesignMark => ({ ids: u.ids, point: [(u.bounds[0][0] + u.bounds[1][0]) / 2, (u.bounds[0][1] + u.bounds[1][1]) / 2] }));
  return closestPair(marks.map((m) => m.point)) < MIN_MARK_DISTANCE_MM ? { error: 'close' } : { marks };
}

/** ¿Alguna marca se movió o se borró desde que se midió? El registro ya no vale para ese diseño. */
export function marksMoved(doc: Pick<Document, 'objects'>, marks: readonly DesignMark[], tolMm = 0.01): boolean {
  const byId = new Map(doc.objects.map((o) => [o.id, o]));
  return marks.some((m) => {
    const objs = m.ids.map((id) => byId.get(id));
    if (objs.some((o) => !o)) return true;
    const b = unionBounds(objs.map((o) => objectBounds(o!)));
    if (!b) return true;
    return Math.hypot((b[0][0] + b[1][0]) / 2 - m.point[0], (b[0][1] + b[1][1]) / 2 - m.point[1]) > tolMm;
  });
}
