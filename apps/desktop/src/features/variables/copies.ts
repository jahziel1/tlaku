import type { DesignObject, Document } from '@/api/types';
import { isTextObject } from '@/features/text/signature';
import { objectBounds, translateAffine, unionBounds } from '@/lib/geometry';
import { groupIdsOf, regroupCopies } from '@/lib/groups';
import { useStore } from '@/store';
import { uniqueId } from '@/store/document';
import { shiftTemplate } from './syntax';

/** Tope de copias de una vez (más que esto no cabe en ninguna cama). */
export const MAX_COPIES = 500;

export interface CopiesPlan {
  /** Copias en total, contando la selección original (que es la copia 1). */
  count: number;
  /** Copias por fila de la rejilla. */
  cols: number;
  /** Espacio entre copias (mm). */
  gap: number;
}

/**
 * "Una copia por fila": deja la selección donde está (copia 1) y agrega `count - 1` copias en
 * rejilla, hacia la derecha y hacia abajo. En la copia `k`, las variables que avanzan (serie,
 * csv, fila, copia) quedan desplazadas `k` pasos (`{csv:Nombre}` pasa a `{csv:Nombre@k}`): cada
 * etiqueta toma una fila distinta del CSV y, al enviar, todas salen en un solo trabajo.
 * Devuelve el documento nuevo y los ids de las copias creadas.
 */
export function copiesPerRow(doc: Document, ids: readonly string[], plan: CopiesPlan): { doc: Document; created: string[] } {
  const wanted = new Set(ids);
  const source = doc.objects.filter((o) => wanted.has(o.id));
  const bounds = unionBounds(source.map(objectBounds));
  const count = Math.min(MAX_COPIES, Math.max(1, Math.floor(plan.count)));
  if (!bounds || count < 2) return { doc, created: [] };
  const cols = Math.min(count, Math.max(1, Math.floor(plan.cols)));
  const w = bounds[1][0] - bounds[0][0];
  const h = bounds[1][1] - bounds[0][1];

  const taken = new Set(doc.objects.map((o) => o.id));
  const groups = groupIdsOf(doc.objects);
  const created: string[] = [];
  const copies: DesignObject[] = [];
  for (let k = 1; k < count; k++) {
    const dx = (k % cols) * (w + plan.gap);
    const dy = -Math.floor(k / cols) * (h + plan.gap);
    const batch = source.map((o): DesignObject => {
      const id = uniqueId(o.id.replace(/-\d+$/, ''), taken);
      taken.add(id);
      created.push(id);
      const moved = { ...o, id, locked: false, transform: translateAffine(o.transform, dx, dy) };
      return isTextObject(moved) ? { ...moved, kind: { ...moved.kind, text: shiftTemplate(moved.kind.text, k) } } : moved;
    });
    copies.push(...regroupCopies(batch, groups));
  }
  return { doc: { ...doc, objects: [...doc.objects, ...copies] }, created };
}

/** Aplica `copiesPerRow` a la selección actual con un paso de deshacer y selecciona todo el lote. */
export function makeCopiesPerRow(plan: CopiesPlan): number {
  const s = useStore.getState();
  const ids = s.selectedIds;
  let made: string[] = [];
  s.applyDocChange('variables.history.copies', (doc) => {
    const r = copiesPerRow(doc, ids, plan);
    made = r.created;
    return r.doc;
  });
  if (made.length > 0) s.select([...ids, ...made]);
  return made.length;
}
