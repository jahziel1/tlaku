import type { Document } from '@/api/types';
import { multiplyAffine } from '@/lib/geometry';
import { scaleAbout } from '@/features/canvas/transform';
import { boundsCenter, boundsHeight, boundsWidth, type Unit } from './units';

export type EqualizeAxis = 'width' | 'height';

/**
 * Iguala el ancho (o el alto) de las unidades al de la primera (la de referencia). Cada una se
 * escala alrededor de su propio centro; con `keepRatio` el otro lado escala igual, y si no queda
 * como estaba. Las unidades sin medida en ese eje se omiten. Mismo documento si no hay cambios.
 */
export function equalizeDoc(doc: Document, units: readonly Unit[], axis: EqualizeAxis, keepRatio: boolean): Document {
  const ref = units[0];
  if (!ref || units.length < 2) return doc;
  const target = axis === 'width' ? boundsWidth(ref.bounds) : boundsHeight(ref.bounds);
  if (target < 1e-9) return doc;
  const matrices = new Map<string, ReturnType<typeof scaleAbout>>();
  for (const u of units.slice(1)) {
    const size = axis === 'width' ? boundsWidth(u.bounds) : boundsHeight(u.bounds);
    if (size < 1e-9) continue;
    const k = target / size;
    if (Math.abs(k - 1) < 1e-9) continue;
    const other = keepRatio ? k : 1;
    const m = scaleAbout(boundsCenter(u.bounds), axis === 'width' ? k : other, axis === 'width' ? other : k);
    for (const id of u.ids) matrices.set(id, m);
  }
  if (matrices.size === 0) return doc;
  return {
    ...doc,
    objects: doc.objects.map((o) => {
      const m = matrices.get(o.id);
      return m && !o.locked ? { ...o, transform: multiplyAffine(m, o.transform) } : o;
    }),
  };
}
