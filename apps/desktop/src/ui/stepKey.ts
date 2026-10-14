/**
 * Paso con flechas de un campo numérico: ↑ suma y ↓ resta `step`; con Mayús el paso es 10 veces
 * mayor y con Alt, 10 veces menor. Devuelve el valor nuevo (dentro de `min`..`max`) o `null` si la
 * tecla no es ↑/↓ (para que el llamador no la consuma).
 */
export function stepFromKey(
  e: { key: string; shiftKey: boolean; altKey: boolean },
  current: number,
  step: number,
  min = -Infinity,
  max = Infinity,
): number | null {
  if (e.key !== 'ArrowUp' && e.key !== 'ArrowDown') return null;
  const k = e.shiftKey ? 10 : e.altKey ? 0.1 : 1;
  const next = current + (e.key === 'ArrowUp' ? 1 : -1) * step * k;
  // Se redondea para que 0,1 + 0,2 no dé 0,30000000000000004.
  return Math.min(max, Math.max(min, Math.round(next * 1e6) / 1e6));
}
