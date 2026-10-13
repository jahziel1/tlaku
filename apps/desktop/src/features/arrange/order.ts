import type { DesignObject } from '@/api/types';

/** Orden de dibujo: el último objeto de la lista se dibuja encima de los demás. */
export type OrderOp = 'front' | 'back' | 'forward' | 'backward';

/**
 * Nueva lista con los objetos `ids` movidos según `op`. Los elegidos conservan su orden relativo.
 * Devuelve la misma lista (misma referencia) si el orden no cambia.
 */
export function reorderObjects(objects: readonly DesignObject[], ids: ReadonlySet<string>, op: OrderOp): readonly DesignObject[] {
  const picked = objects.filter((o) => ids.has(o.id));
  if (picked.length === 0 || picked.length === objects.length) return objects;
  let next: DesignObject[];
  if (op === 'front') {
    next = [...objects.filter((o) => !ids.has(o.id)), ...picked];
  } else if (op === 'back') {
    next = [...picked, ...objects.filter((o) => !ids.has(o.id))];
  } else {
    // Un paso: cada elegido intercambia lugar con su vecino no elegido (de arriba abajo al subir).
    next = [...objects];
    if (op === 'forward') {
      for (let i = next.length - 2; i >= 0; i--) {
        if (ids.has(next[i]!.id) && !ids.has(next[i + 1]!.id)) [next[i], next[i + 1]] = [next[i + 1]!, next[i]!];
      }
    } else {
      for (let i = 1; i < next.length; i++) {
        if (ids.has(next[i]!.id) && !ids.has(next[i - 1]!.id)) [next[i], next[i - 1]] = [next[i - 1]!, next[i]!];
      }
    }
  }
  return next.every((o, i) => o === objects[i]) ? objects : next;
}
