/**
 * Ventana flotante de la vista en vivo (`features/live/LiveWindow.tsx`): posición y tamaño dentro de
 * la ventana de la app, y cómo se recuerdan (preferencia `liveWindow`). Sin DOM: todo con números.
 */

export interface LiveRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Lo que se recuerda: el rectángulo «normal» y si estaba agrandada (el rectángulo se conserva para restaurar). */
export interface LiveWindowPref extends LiveRect {
  max: boolean;
}

/** Tamaño mínimo (px): cabe la barra de título, la cama y una línea con porcentaje y tiempos. */
export const LIVE_MIN_W = 280;
export const LIVE_MIN_H = 220;
export const LIVE_MAX_W = 3000;
export const LIVE_MAX_H = 2400;
/** Tamaño con que aparece la primera vez. */
export const LIVE_DEFAULT = { w: 420, h: 340 } as const;
/** Separación (px) a los bordes del área al aparecer y al agrandarse. */
export const LIVE_MARGIN = 12;

const finite = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/** Lo guardado, saneado: medidas dentro de los límites y enteras. `null` si no sirve. */
export function sanitizeLiveWindow(raw: unknown): LiveWindowPref | null {
  const r = raw && typeof raw === 'object' ? (raw as Record<string, unknown>) : null;
  if (!r || !finite(r.x) || !finite(r.y) || !finite(r.w) || !finite(r.h)) return null;
  return {
    x: Math.round(clamp(r.x, 0, 20000)),
    y: Math.round(clamp(r.y, 0, 20000)),
    w: Math.round(clamp(r.w, LIVE_MIN_W, LIVE_MAX_W)),
    h: Math.round(clamp(r.h, LIVE_MIN_H, LIVE_MAX_H)),
    max: r.max === true,
  };
}

/**
 * Rectángulo que cabe entero en `bounds` (el tamaño de la ventana de la app): si la ventana se
 * achica, la flotante se reacomoda y nunca queda parte fuera. Nunca baja del tamaño mínimo (si la
 * app es más chica que eso, queda pegada a la esquina).
 */
export function fitLiveRect(rect: LiveRect, bounds: { w: number; h: number }): LiveRect {
  const w = Math.min(rect.w, Math.max(LIVE_MIN_W, bounds.w));
  const h = Math.min(rect.h, Math.max(LIVE_MIN_H, bounds.h));
  return { w, h, x: Math.round(clamp(rect.x, 0, Math.max(0, bounds.w - w))), y: Math.round(clamp(rect.y, 0, Math.max(0, bounds.h - h))) };
}

/** Cambia el tamaño desde la esquina inferior derecha, sin pasar de los límites ni salirse de `bounds`. */
export function resizeLiveRect(from: LiveRect, dx: number, dy: number, bounds: { w: number; h: number }): LiveRect {
  const maxW = Math.max(LIVE_MIN_W, Math.min(LIVE_MAX_W, bounds.w - from.x));
  const maxH = Math.max(LIVE_MIN_H, Math.min(LIVE_MAX_H, bounds.h - from.y));
  return { ...from, w: Math.round(clamp(from.w + dx, LIVE_MIN_W, maxW)), h: Math.round(clamp(from.h + dy, LIVE_MIN_H, maxH)) };
}

/** Dónde aparece la primera vez: en la esquina inferior derecha de `area` (el lienzo) con un margen. */
export function defaultLiveRect(area: LiveRect): LiveRect {
  const w = Math.min(LIVE_DEFAULT.w, Math.max(LIVE_MIN_W, area.w - 2 * LIVE_MARGIN));
  const h = Math.min(LIVE_DEFAULT.h, Math.max(LIVE_MIN_H, area.h - 2 * LIVE_MARGIN));
  return { w, h, x: Math.round(area.x + area.w - w - LIVE_MARGIN), y: Math.round(area.y + area.h - h - LIVE_MARGIN) };
}

/** «Agrandada»: casi toda `area`, con un margen. */
export function maxLiveRect(area: LiveRect): LiveRect {
  return { x: area.x + LIVE_MARGIN, y: area.y + LIVE_MARGIN, w: Math.max(LIVE_MIN_W, area.w - 2 * LIVE_MARGIN), h: Math.max(LIVE_MIN_H, area.h - 2 * LIVE_MARGIN) };
}
