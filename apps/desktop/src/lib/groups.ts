import type { DesignObject, Document } from '@/api/types';

/**
 * Grupos planos: los objetos con el mismo `group` se seleccionan, mueven y escalan juntos. Aquí
 * solo viven las consultas puras; agrupar y desagrupar son comandos (`edit.group`/`edit.ungroup`).
 */

/** Los `ids` más los demás miembros de sus grupos (que cumplan `keep`), sin repetir y en orden estable. */
export function expandGroups(
  objects: readonly DesignObject[],
  ids: readonly string[],
  keep: (o: DesignObject) => boolean = () => true,
): string[] {
  const wanted = new Set(ids);
  const groups = new Set<string>();
  for (const o of objects) if (o.group && wanted.has(o.id)) groups.add(o.group);
  if (groups.size === 0) return [...wanted];
  const out = new Set(ids);
  for (const o of objects) if (o.group && groups.has(o.group) && keep(o)) out.add(o.id);
  return [...out];
}

/** Ids de los miembros de un grupo, en el orden del documento. */
export function groupMembers(objects: readonly DesignObject[], group: string): string[] {
  return objects.filter((o) => o.group === group).map((o) => o.id);
}

/** Ids de grupo en uso. */
export function groupIdsOf(objects: readonly DesignObject[]): Set<string> {
  const out = new Set<string>();
  for (const o of objects) if (o.group) out.add(o.group);
  return out;
}

/** ¿Alguno de los `ids` pertenece a un grupo? */
export function hasGroup(objects: readonly DesignObject[], ids: readonly string[]): boolean {
  const wanted = new Set(ids);
  return objects.some((o) => wanted.has(o.id) && o.group !== null);
}

function freshGroupId(taken: Set<string>): string {
  let i = 1;
  while (taken.has(`grupo-${i}`)) i++;
  const id = `grupo-${i}`;
  taken.add(id);
  return id;
}

/**
 * Agrupa los objetos no bloqueados de `ids` en un grupo nuevo (los grupos son planos: si ya
 * estaban en otros, pasan a este). Un grupo que se quede con un solo miembro se disuelve.
 * Devuelve el mismo documento si no hay al menos dos objetos que agrupar.
 */
export function groupDoc(doc: Document, ids: readonly string[]): Document {
  const wanted = new Set(ids);
  const targets = doc.objects.filter((o) => wanted.has(o.id) && !o.locked);
  if (targets.length < 2) return doc;
  const id = freshGroupId(groupIdsOf(doc.objects));
  const moved = new Set(targets.map((o) => o.id));
  const left = new Map<string, number>(); // miembros que se quedan en cada grupo viejo
  for (const o of doc.objects) if (o.group && !moved.has(o.id)) left.set(o.group, (left.get(o.group) ?? 0) + 1);
  return {
    ...doc,
    objects: doc.objects.map((o) => {
      if (moved.has(o.id)) return { ...o, group: id };
      if (o.group && left.get(o.group) === 1) return { ...o, group: null };
      return o;
    }),
  };
}

/** Deshace los grupos de los objetos de `ids` (disuelve el grupo entero). Mismo documento si no hay ninguno. */
export function ungroupDoc(doc: Document, ids: readonly string[]): Document {
  const wanted = new Set(ids);
  const groups = new Set<string>();
  for (const o of doc.objects) if (o.group && wanted.has(o.id)) groups.add(o.group);
  if (groups.size === 0) return doc;
  return { ...doc, objects: doc.objects.map((o) => (o.group && groups.has(o.group) ? { ...o, group: null } : o)) };
}

/**
 * Copias (duplicar, pegar) con grupos propios: cada grupo de origen pasa a un grupo nuevo, así las
 * copias no se unen al original. Un objeto cuyo grupo no se copió completo queda sin grupo.
 */
export function regroupCopies(copies: DesignObject[], taken: Set<string>): DesignObject[] {
  const size = new Map<string, number>();
  for (const o of copies) if (o.group) size.set(o.group, (size.get(o.group) ?? 0) + 1);
  const fresh = new Map<string, string>();
  return copies.map((o) => {
    if (!o.group) return o;
    if ((size.get(o.group) ?? 0) < 2) return { ...o, group: null };
    let id = fresh.get(o.group);
    if (!id) {
      id = freshGroupId(taken);
      fresh.set(o.group, id);
    }
    return { ...o, group: id };
  });
}
