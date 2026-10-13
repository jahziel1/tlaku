import type { DesignObject, Point } from '@/api/types';
import { applyAffine, objectPolylines, type Bounds } from '@/lib/geometry';
import type { NestPlacement } from './types';

/** Aplanado grueso: la vista previa no necesita la precisión del corte. */
const PREVIEW_TOLERANCE_MM = 0.1;

export interface FlatPoly {
  points: Point[];
  closed: boolean;
}

/** Contornos de una pieza (todos sus objetos) en coordenadas de documento, ya aplanados. */
export function flatPolylines(objects: readonly DesignObject[]): FlatPoly[] {
  return objects.flatMap((o) => objectPolylines(o, PREVIEW_TOLERANCE_MM));
}

const num = (n: number): string => (Math.round(n * 100) / 100).toString();

/** Datos de un `<path>`: cada contorno como "M ... L ... Z" (con `Z` solo si es cerrado). */
export function pathData(polys: readonly FlatPoly[], place?: NestPlacement['transform']): string {
  const parts: string[] = [];
  for (const { points, closed } of polys) {
    points.forEach((p, i) => {
      const [x, y] = place ? applyAffine(place, p) : p;
      parts.push(`${i === 0 ? 'M' : 'L'}${num(x)} ${num(y)}`);
    });
    if (closed && points.length > 0) parts.push('Z');
  }
  return parts.join('');
}

export interface PreviewPiece {
  key: string;
  d: string;
  /** 0 = original, >= 1 = copia nueva. */
  copy: number;
}

/** Contornos de cada pieza colocada con el `transform` del acomodo por encima del que ya tenían. */
export function placedPieces(
  flat: ReadonlyMap<string, readonly FlatPoly[]>,
  placements: readonly NestPlacement[],
): { pieces: PreviewPiece[]; bounds: Bounds | null } {
  const pieces: PreviewPiece[] = [];
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const p of placements) {
    const polys = flat.get(p.piece);
    if (!polys) continue;
    for (const { points } of polys) {
      for (const pt of points) {
        const [x, y] = applyAffine(p.transform, pt);
        if (x < minX) minX = x;
        if (x > maxX) maxX = x;
        if (y < minY) minY = y;
        if (y > maxY) maxY = y;
      }
    }
    pieces.push({ key: `${p.piece}#${p.copy}`, d: pathData(polys, p.transform), copy: p.copy });
  }
  const bounds: Bounds | null = minX <= maxX ? [[minX, minY], [maxX, maxY]] : null;
  return { pieces, bounds };
}

/** Cuenta de piezas que no cupieron por pieza, en el orden en que aparecen. */
export function unplacedSummary(unplaced: readonly { piece: string }[]): { piece: string; count: number }[] {
  const counts = new Map<string, number>();
  for (const u of unplaced) counts.set(u.piece, (counts.get(u.piece) ?? 0) + 1);
  return [...counts].map(([piece, count]) => ({ piece, count }));
}
