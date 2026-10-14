import type { DesignObject } from '@/api/types';

/**
 * Posiciones que cambiaron entre dos versiones de la lista de objetos (los objetos son inmutables:
 * cambiar es reemplazar la referencia). Solo describe cambios "por posición": misma longitud, o `next`
 * con objetos añadidos al final. Devuelve `null` si hay objetos quitados o más de `limit` cambios (quien
 * llama debe rehacer todo); así el costo es O(n) comparaciones de referencia sin asignar nada, y
 * termina en cuanto supera `limit`.
 */
export function diffObjects(prev: readonly DesignObject[], next: readonly DesignObject[], limit: number): number[] | null {
  if (next.length < prev.length) return null;
  const changed: number[] = [];
  for (let i = 0; i < prev.length; i++) {
    if (prev[i] === next[i]) continue;
    if (changed.length >= limit) return null;
    changed.push(i);
  }
  if (changed.length + (next.length - prev.length) > limit) return null;
  for (let i = prev.length; i < next.length; i++) changed.push(i);
  return changed;
}
