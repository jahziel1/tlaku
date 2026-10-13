import type { Point } from '@/api/types';
import type { Bounds } from '@/lib/geometry';
import { worldToScreen, type ViewTransform } from '@/lib/viewport';
import { HANDLES, isCornerHandle, type HandleId } from './transform';

/** Holgura entre la caja de la selección y su marco, en px. */
export const FRAME_PAD = 6;
/** Radio de captura de una manija, en px (la manija se dibuja de 8 px). */
export const HANDLE_HIT = 8;
/** Distancia de la manija de rotación sobre el borde superior, en px. */
export const ROTATE_OFFSET = 24;
/** Lado mínimo del marco para mostrar manijas de lado sin que se encimen con las esquinas. */
const MIN_SIDE_FOR_MIDDLE = 30;

export type HandleKind = 'scale' | 'rotate';

export interface HandleSpot {
  id: HandleId | 'knob';
  kind: HandleKind;
  x: number;
  y: number;
}

export interface ScreenRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Marco de la selección en pantalla (con holgura). */
export function frameRect(b: Bounds, view: ViewTransform): ScreenRect {
  const [x0, y0] = worldToScreen(view, [b[0][0], b[1][1]]);
  const [x1, y1] = worldToScreen(view, [b[1][0], b[0][1]]);
  return { x: x0 - FRAME_PAD, y: y0 - FRAME_PAD, w: x1 - x0 + FRAME_PAD * 2, h: y1 - y0 + FRAME_PAD * 2 };
}

function spotOf(r: ScreenRect, id: HandleId): Point {
  const fx = id.includes('w') ? 0 : id.includes('e') ? 1 : 0.5;
  const fy = id.includes('n') ? 0 : id.includes('s') ? 1 : 0.5;
  return [r.x + r.w * fx, r.y + r.h * fy];
}

/**
 * Manijas visibles. En modo giro (segundo clic) las esquinas giran y los lados desaparecen;
 * en modo normal las esquinas y los lados escalan y la perilla superior gira.
 */
export function handleSpots(b: Bounds, view: ViewTransform, opts: { rotateMode: boolean; canRotate: boolean }): HandleSpot[] {
  const r = frameRect(b, view);
  const spots: HandleSpot[] = [];
  for (const id of HANDLES) {
    const corner = isCornerHandle(id);
    if (opts.rotateMode && !corner) continue;
    if (!corner) {
      const horizontalSide = id === 'n' || id === 's';
      if ((horizontalSide ? r.w : r.h) < MIN_SIDE_FOR_MIDDLE) continue;
    }
    if (opts.rotateMode && !opts.canRotate) continue;
    const [x, y] = spotOf(r, id);
    spots.push({ id, kind: opts.rotateMode ? 'rotate' : 'scale', x, y });
  }
  if (!opts.rotateMode && opts.canRotate) {
    spots.push({ id: 'knob', kind: 'rotate', x: r.x + r.w / 2, y: r.y - ROTATE_OFFSET });
  }
  return spots;
}

export function hitHandle(spots: readonly HandleSpot[], sx: number, sy: number): HandleSpot | null {
  let best: HandleSpot | null = null;
  let bestD = HANDLE_HIT * HANDLE_HIT;
  for (const s of spots) {
    const d = (s.x - sx) ** 2 + (s.y - sy) ** 2;
    if (d <= bestD) {
      best = s;
      bestD = d;
    }
  }
  return best;
}

/** Cursor CSS para cada manija (los de giro se resuelven con la clase `rotate`). */
export function cursorForHandle(spot: HandleSpot): string {
  if (spot.kind === 'rotate') return 'rotate';
  switch (spot.id) {
    case 'n': case 's': return 'ns-resize';
    case 'e': case 'w': return 'ew-resize';
    case 'nw': case 'se': return 'nwse-resize';
    default: return 'nesw-resize';
  }
}
