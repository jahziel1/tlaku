/**
 * Zona del recorrido de captura (M36): un rectángulo que el usuario dibuja sobre el lienzo. Aquí
 * solo la geometría; el dibujo sobre el lienzo está en `ZoneOverlay.tsx`.
 */
import type { Vec2 } from './geometry';
import type { ZoneRect } from './headState';

/** Lado mínimo (mm) de una zona dibujada: menos que eso es un clic, no una zona. */
export const MIN_ZONE_MM = 2;

/** Rectángulo normalizado entre dos esquinas del arrastre, recortado a la cama; `null` si es muy chico. */
export function rectFromDrag(a: Vec2, b: Vec2, bed: Vec2): ZoneRect | null {
  const clamp = (v: number, hi: number) => Math.min(hi, Math.max(0, v));
  const x0 = clamp(Math.min(a[0], b[0]), bed[0]);
  const x1 = clamp(Math.max(a[0], b[0]), bed[0]);
  const y0 = clamp(Math.min(a[1], b[1]), bed[1]);
  const y1 = clamp(Math.max(a[1], b[1]), bed[1]);
  if (![x0, x1, y0, y1].every(Number.isFinite) || x1 - x0 < MIN_ZONE_MM || y1 - y0 < MIN_ZONE_MM) return null;
  return [x0, y0, x1, y1];
}

export const zoneSize = (z: ZoneRect): Vec2 => [z[2] - z[0], z[3] - z[1]];

/** Recorta una zona guardada a la cama actual (cambió de máquina); `null` si ya no queda nada. */
export function clipZone(z: ZoneRect, bed: Vec2): ZoneRect | null {
  return rectFromDrag([z[0], z[1]], [z[2], z[3]], bed);
}
