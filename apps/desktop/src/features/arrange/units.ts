import type { DesignObject, Document } from '@/api/types';
import { objectBounds, unionBounds, type Bounds } from '@/lib/geometry';

/**
 * Unidad de organización: un objeto suelto o un grupo completo (un grupo se alinea, distribuye
 * y combina como una sola cosa). Las unidades bloqueadas no participan: no se pueden mover.
 */
export interface Unit {
  /** Id de referencia de la unidad: el del primer miembro. */
  id: string;
  objects: DesignObject[];
  ids: string[];
  bounds: Bounds;
}

/**
 * Unidades de la selección en el orden en que se eligieron (la primera aparición de cada
 * objeto o grupo). Se descartan los bloqueados, los ocultos y los que no dibujan nada.
 */
export function selectionUnits(doc: Pick<Document, 'objects'>, selectedIds: readonly string[]): Unit[] {
  const byId = new Map(doc.objects.map((o) => [o.id, o]));
  const wanted = new Set(selectedIds);
  const groups = new Set<string>();
  for (const o of doc.objects) if (o.group && wanted.has(o.id)) groups.add(o.group);

  const seen = new Set<string>();
  const units: Unit[] = [];
  for (const id of selectedIds) {
    const o = byId.get(id);
    if (!o || seen.has(o.id)) continue;
    const members = o.group && groups.has(o.group) ? doc.objects.filter((m) => m.group === o.group) : [o];
    for (const m of members) seen.add(m.id);
    if (members.some((m) => m.locked || m.hidden)) continue;
    const bounds = unionBounds(members.map(objectBounds));
    if (!bounds) continue;
    units.push({ id: members[0]!.id, objects: members, ids: members.map((m) => m.id), bounds });
  }
  return units;
}

export const boundsWidth = (b: Bounds): number => b[1][0] - b[0][0];
export const boundsHeight = (b: Bounds): number => b[1][1] - b[0][1];
export const boundsCenter = (b: Bounds): [number, number] => [(b[0][0] + b[1][0]) / 2, (b[0][1] + b[1][1]) / 2];

/** Caja que envuelve todas las unidades (el bloque que se copia en un arreglo). */
export function blockBounds(units: readonly Unit[]): Bounds | null {
  return unionBounds(units.map((u) => u.bounds));
}

export const unitSize = (u: Unit): [number, number] => [boundsWidth(u.bounds), boundsHeight(u.bounds)];
