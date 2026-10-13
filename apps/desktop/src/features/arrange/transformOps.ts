import type { Affine, DesignObject, Document, Point } from '@/api/types';
import { applyAffine, flipHorizontalAbout, flipVerticalAbout, multiplyAffine, type Bounds } from '@/lib/geometry';
import { regroupCopies, groupIdsOf } from '@/lib/groups';
import { boundsCenter, boundsOf, rotateAbout } from '@/features/canvas/transform';
import { uniqueId } from '@/store/document';

/** Caja que envuelve a los objetos movibles (no bloqueados) de la selección. */
export function movableBounds(objects: readonly DesignObject[]): Bounds | null {
  return boundsOf(objects.filter((o) => !o.locked && !o.hidden));
}

/** Matriz de voltear la selección sobre el centro de su caja (izquierda-derecha o arriba-abajo). */
export function flipMatrix(objects: readonly DesignObject[], axis: 'h' | 'v'): Affine | null {
  const b = movableBounds(objects);
  if (!b) return null;
  const [cx, cy] = boundsCenter(b);
  return axis === 'h' ? flipHorizontalAbout(cx) : flipVerticalAbout(cy);
}

/** Matriz de girar `degrees` (antihorario) alrededor del centro de la caja de la selección. */
export function rotateSelectionMatrix(objects: readonly DesignObject[], degrees: number): Affine | null {
  const b = movableBounds(objects);
  if (!b || degrees === 0) return null;
  return rotateAbout(boundsCenter(b), (degrees * Math.PI) / 180);
}

/**
 * Extremos de un objeto que es una recta de exactamente dos puntos (la línea de un espejo), en
 * coordenadas de documento; `null` si es otra cosa.
 */
export function straightLineEnds(obj: DesignObject): [Point, Point] | null {
  if (obj.kind.type !== 'vector' || obj.kind.paths.length !== 1) return null;
  const path = obj.kind.paths[0]!;
  if (path.length !== 2 || path[0]!.type !== 'move_to' || path[1]!.type !== 'line_to') return null;
  const [a, b] = [path[0].to, path[1].to].map((p) => applyAffine(obj.transform, p as Point)) as [Point, Point];
  return Math.hypot(b[0] - a[0], b[1] - a[1]) < 1e-9 ? null : [a, b];
}

/**
 * Agrega copias reflejadas por `matrix` de los objetos `ids` (no bloqueados) y devuelve los ids
 * nuevos. Las copias conservan la capa y forman sus propios grupos. Mismo documento si no hay nada.
 */
export function mirrorCopiesDoc(doc: Document, ids: readonly string[], matrix: Affine): { doc: Document; created: string[] } {
  const wanted = new Set(ids);
  const sources = doc.objects.filter((o) => wanted.has(o.id) && !o.locked && !o.hidden);
  if (sources.length === 0) return { doc, created: [] };
  const taken = new Set(doc.objects.map((o) => o.id));
  const created: string[] = [];
  const copies = regroupCopies(
    sources.map((o) => {
      const id = uniqueId(o.id.replace(/-\d+$/, ''), taken);
      taken.add(id);
      created.push(id);
      return { ...o, id, locked: false, transform: multiplyAffine(matrix, o.transform) };
    }),
    groupIdsOf(doc.objects),
  );
  return { doc: { ...doc, objects: [...doc.objects, ...copies] }, created };
}
