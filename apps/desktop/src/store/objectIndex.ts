import type { DesignObject } from '@/api/types';

/**
 * Índice id -> posición de la lista de objetos del documento.
 *
 * Los arreglos de objetos son inmutables (cada cambio crea uno nuevo), así que el índice se guarda
 * por identidad del arreglo y se calcula una vez (O(n)). Las acciones que no cambian el orden ni los
 * ids (mover, cambiar de capa) llaman a `shareIndex` para que el arreglo nuevo herede el índice sin
 * recorrerlo: con 100 000 objetos, mover unos pocos deja de costar un recorrido completo.
 */
export interface ObjectIndex {
  /** Posición de cada id (la primera, si el id se repite). */
  at: ReadonlyMap<string, number>;
  /** Todos los ids son distintos: solo así `at` describe la lista completa. */
  unique: boolean;
}

const cache = new WeakMap<readonly DesignObject[], ObjectIndex>();

/** Índice de `objects`, construido en el primer uso. */
export function objectIndex(objects: readonly DesignObject[]): ObjectIndex {
  let idx = cache.get(objects);
  if (!idx) {
    const at = new Map<string, number>();
    let unique = true;
    for (let i = 0; i < objects.length; i++) {
      const id = objects[i]!.id;
      if (at.has(id)) unique = false;
      else at.set(id, i);
    }
    idx = { at, unique };
    cache.set(objects, idx);
  }
  return idx;
}

/** `to` tiene los mismos ids en el mismo orden que `from`: hereda su índice (si `from` ya lo tiene). */
export function shareIndex(from: readonly DesignObject[], to: readonly DesignObject[]): void {
  const idx = cache.get(from);
  if (idx && to.length === from.length) cache.set(to, idx);
}

const byIdsCache = new WeakMap<readonly string[], { objects: readonly DesignObject[]; out: DesignObject[] }>();

/**
 * Objetos con esos ids (en el orden de la lista del documento, sin repetidos ni ids ajenos). El resultado se
 * guarda por arreglo de ids (la selección del store cambia de identidad al cambiar) y de objetos, así que quien
 * lo pide varias veces por cuadro (barras, barra de estado, gestos) comparte el cálculo. No hay que modificarlo.
 */
export function objectsByIds(objects: readonly DesignObject[], ids: readonly string[]): DesignObject[] {
  const hit = byIdsCache.get(ids);
  if (hit && hit.objects === objects) return hit.out;
  const idx = objectIndex(objects);
  let out: DesignObject[];
  // Con muchos ids, ordenar costaría más que recorrer la lista una vez.
  if (!idx.unique || ids.length * 8 > objects.length) {
    const set = new Set(ids);
    out = objects.filter((o) => set.has(o.id));
  } else {
    const found: number[] = [];
    const seen = new Set<number>();
    for (const id of ids) {
      const i = idx.at.get(id);
      if (i !== undefined && !seen.has(i)) {
        seen.add(i);
        found.push(i);
      }
    }
    found.sort((a, b) => a - b);
    out = found.map((i) => objects[i]!);
  }
  byIdsCache.set(ids, { objects, out });
  return out;
}
