import type { DesignObject, Document } from '@/api/types';

/**
 * Grupos anidados. El contrato (`DesignObject.group`) guarda una sola cadena por objeto, así que la
 * jerarquía va dentro de ella: `"grupo-1/grupo-3"` es el objeto del grupo `grupo-3`, que a su vez
 * está dentro de `grupo-1` (de afuera hacia adentro). Un archivo con grupos planos (`"g1"`) es el
 * caso de un solo nivel. El motor solo compara estas cadenas por igualdad y no las interpreta.
 *
 * Al hacer clic se elige siempre el grupo de más afuera ("un grupo cuenta como un solo objeto");
 * desagrupar quita un nivel cada vez. Aquí solo viven las consultas y operaciones puras; agrupar y
 * desagrupar son comandos (`edit.group`/`edit.ungroup`).
 */

const SEP = '/';

/** Cadena de grupos de un objeto, de afuera hacia adentro (vacía si no tiene). */
export function groupChain(group: string | null): string[] {
  return group ? group.split(SEP).filter(Boolean) : [];
}

function chainToGroup(chain: readonly string[]): string | null {
  return chain.length ? chain.join(SEP) : null;
}

/** Grupo de más afuera del objeto, el que manda al seleccionar, mover y alinear. */
export function topGroup(group: string | null): string | null {
  return groupChain(group)[0] ?? null;
}

/** Los `ids` más los demás miembros de sus grupos de más afuera (que cumplan `keep`), sin repetir y en orden estable. */
export function expandGroups(
  objects: readonly DesignObject[],
  ids: readonly string[],
  keep: (o: DesignObject) => boolean = () => true,
): string[] {
  const wanted = new Set(ids);
  const groups = new Set<string>();
  for (const o of objects) {
    const top = topGroup(o.group);
    if (top && wanted.has(o.id)) groups.add(top);
  }
  if (groups.size === 0) return [...wanted];
  const out = new Set(ids);
  for (const o of objects) {
    const top = topGroup(o.group);
    if (top && groups.has(top) && keep(o)) out.add(o.id);
  }
  return [...out];
}

/** Ids de los miembros de un grupo (a cualquier profundidad), en el orden del documento. */
export function groupMembers(objects: readonly DesignObject[], group: string): string[] {
  return objects.filter((o) => groupChain(o.group).includes(group)).map((o) => o.id);
}

/** Ids de grupo en uso, de todos los niveles. */
export function groupIdsOf(objects: readonly DesignObject[]): Set<string> {
  const out = new Set<string>();
  for (const o of objects) for (const g of groupChain(o.group)) out.add(g);
  return out;
}

/** ¿Alguno de los `ids` pertenece a un grupo? */
export function hasGroup(objects: readonly DesignObject[], ids: readonly string[]): boolean {
  const wanted = new Set(ids);
  return objects.some((o) => wanted.has(o.id) && o.group !== null);
}

/** Cuántas "unidades" hay en `ids`: grupos de más afuera completos y objetos sueltos. */
export function countUnits(objects: readonly DesignObject[], ids: readonly string[]): number {
  const wanted = new Set(ids);
  const units = new Set<string>();
  for (const o of objects) {
    if (o.locked || !wanted.has(o.id)) continue;
    units.add(topGroup(o.group) ?? `#${o.id}`);
  }
  return units.size;
}

function freshGroupId(taken: Set<string>): string {
  let i = 1;
  while (taken.has(`grupo-${i}`)) i++;
  const id = `grupo-${i}`;
  taken.add(id);
  return id;
}

/**
 * Quita los niveles que se quedaron con un solo miembro (un grupo de uno no es un grupo). Se mira
 * cada prefijo de la cadena: el prefijo que solo comparte un objeto se descarta. Devuelve los
 * mismos objetos (misma referencia) si no hay nada que corregir.
 */
function pruneSingletons(objects: DesignObject[]): DesignObject[] {
  const count = new Map<string, number>();
  for (const o of objects) {
    const chain = groupChain(o.group);
    for (let i = 1; i <= chain.length; i++) {
      const key = chain.slice(0, i).join(SEP);
      count.set(key, (count.get(key) ?? 0) + 1);
    }
  }
  return objects.map((o) => {
    const chain = groupChain(o.group);
    if (chain.length === 0) return o;
    const kept = chain.filter((_, i) => (count.get(chain.slice(0, i + 1).join(SEP)) ?? 0) >= 2);
    return kept.length === chain.length ? o : { ...o, group: chainToGroup(kept) };
  });
}

/**
 * Agrupa los objetos no bloqueados de `ids` en un grupo nuevo. Un grupo que está completo entre
 * los `ids` entra como una unidad (queda anidado); si solo estaban algunos de sus miembros, esos
 * salen de su grupo y pasan al nuevo. Un grupo que se quede con un solo miembro se disuelve.
 * Devuelve el mismo documento si no hay al menos dos unidades que agrupar.
 */
export function groupDoc(doc: Document, ids: readonly string[]): Document {
  const wanted = new Set(ids);
  const targets = doc.objects.filter((o) => wanted.has(o.id) && !o.locked);
  const picked = new Set(targets.map((o) => o.id));
  // ¿Está el grupo de más afuera completo (entre los que se pueden mover)?
  const members = new Map<string, DesignObject[]>();
  for (const o of doc.objects) {
    const top = topGroup(o.group);
    if (top) members.set(top, [...(members.get(top) ?? []), o]);
  }
  const whole = (o: DesignObject): boolean => {
    const top = topGroup(o.group);
    return top !== null && members.get(top)!.every((m) => picked.has(m.id));
  };
  const chains = new Map<string, string[]>(targets.map((o) => [o.id, whole(o) ? groupChain(o.group) : []]));
  const units = new Set(targets.map((o) => chains.get(o.id)![0] ?? `#${o.id}`));
  if (units.size < 2) return doc;
  const id = freshGroupId(groupIdsOf(doc.objects));
  const objects = doc.objects.map((o) => {
    const chain = chains.get(o.id);
    return chain ? { ...o, group: chainToGroup([id, ...chain]) } : o;
  });
  return { ...doc, objects: pruneSingletons(objects) };
}

/**
 * Desagrupa un nivel: los objetos de `ids` salen del grupo de más afuera al que pertenecen (el
 * grupo entero se abre; si tenía subgrupos, quedan como grupos). Mismo documento si no hay ninguno.
 */
export function ungroupDoc(doc: Document, ids: readonly string[]): Document {
  const wanted = new Set(ids);
  const tops = new Set<string>();
  for (const o of doc.objects) {
    const top = topGroup(o.group);
    if (top && wanted.has(o.id)) tops.add(top);
  }
  if (tops.size === 0) return doc;
  const objects = doc.objects.map((o) => {
    const top = topGroup(o.group);
    return top && tops.has(top) ? { ...o, group: chainToGroup(groupChain(o.group).slice(1)) } : o;
  });
  return { ...doc, objects: pruneSingletons(objects) };
}

/**
 * Copias (duplicar, pegar) con grupos propios: cada grupo de origen pasa a un grupo nuevo, así las
 * copias no se unen al original. Un nivel cuyo grupo no se copió completo (menos de dos miembros) se descarta.
 */
export function regroupCopies(copies: DesignObject[], taken: Set<string>): DesignObject[] {
  const size = new Map<string, number>();
  for (const o of copies) for (const g of groupChain(o.group)) size.set(g, (size.get(g) ?? 0) + 1);
  const fresh = new Map<string, string>();
  return copies.map((o) => {
    if (!o.group) return o;
    const chain = groupChain(o.group)
      .filter((g) => (size.get(g) ?? 0) >= 2)
      .map((g) => {
        let id = fresh.get(g);
        if (!id) {
          id = freshGroupId(taken);
          fresh.set(g, id);
        }
        return id;
      });
    return { ...o, group: chainToGroup(chain) };
  });
}
