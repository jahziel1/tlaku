import type { Point } from '@/api/types';
import type { Bounds } from './geometry';
import { MM_PER_INCH, formatLength, type LengthUnit } from './units';

/**
 * Relación entre el espacio del documento (mm, Y hacia arriba) y el de pantalla (px, Y hacia abajo).
 * `panX/panY` son los px donde cae el origen (0, 0) del documento, medidos desde la esquina
 * superior izquierda del lienzo. `zoom` son px por mm.
 */
export interface ViewTransform {
  zoom: number;
  panX: number;
  panY: number;
}

export const MIN_ZOOM = 0.05;
export const MAX_ZOOM = 400;

export function clampZoom(zoom: number): number {
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, zoom));
}

export function worldToScreen(v: ViewTransform, [x, y]: Point): Point {
  return [v.panX + x * v.zoom, v.panY - y * v.zoom];
}

export function screenToWorld(v: ViewTransform, [sx, sy]: Point): Point {
  return [(sx - v.panX) / v.zoom, (v.panY - sy) / v.zoom];
}

/** Nuevo transform con zoom `factor` veces mayor, manteniendo fijo el punto de pantalla (sx, sy). */
export function zoomAround(v: ViewTransform, factor: number, sx: number, sy: number): ViewTransform {
  const zoom = clampZoom(v.zoom * factor);
  const k = zoom / v.zoom;
  return { zoom, panX: sx - (sx - v.panX) * k, panY: sy - (sy - v.panY) * k };
}

/** Transform que encuadra `bounds` (mm) centrado en un lienzo de `w` x `h` px con margen en px. */
export function fitBounds(bounds: Bounds, w: number, h: number, margin = 32): ViewTransform {
  const bw = Math.max(bounds[1][0] - bounds[0][0], 1e-6);
  const bh = Math.max(bounds[1][1] - bounds[0][1], 1e-6);
  const zoom = clampZoom(Math.min((w - margin * 2) / bw, (h - margin * 2) / bh));
  const cx = (bounds[0][0] + bounds[1][0]) / 2;
  const cy = (bounds[0][1] + bounds[1][1]) / 2;
  return { zoom, panX: w / 2 - cx * zoom, panY: h / 2 + cy * zoom };
}

const STEPS = [0.1, 0.5, 1, 5, 10, 50, 100, 500, 1000];
/** Pasos en pulgadas (décimas, como las reglas de taller): se pasan a mm al usarlos. */
const STEPS_IN = [0.01, 0.05, 0.1, 0.5, 1, 5, 10, 50].map((s) => s * MM_PER_INCH);

/** Menor múltiplo `base × {1, 2, 5} × 10^k` que deja al menos `minPx` entre líneas. */
function fixedMinor(base: number, zoom: number, minPx: number): number {
  for (let k = 0; k < 12; k++) {
    for (const m of [1, 2, 5]) {
      const step = base * m * 10 ** k;
      if (step * zoom >= minPx) return step;
    }
  }
  return base * 10 ** 12;
}

/**
 * Pasos de cuadrícula en mm: `minor` es el más pequeño que deja al menos `minPx` entre
 * líneas; `major` es 10 veces `minor`. Coincide con la regla. En pulgadas los pasos son
 * décimas de pulgada (0,01, 0,05, 0,1, 0,5, 1...), expresados igualmente en mm. Con `baseMm` > 0
 * (preferencia «Rejilla») ese es el paso base y solo se multiplica al alejar el zoom.
 */
export function gridSteps(zoom: number, minPx = 7, unit: LengthUnit = 'mm', baseMm = 0): { minor: number; major: number } {
  if (baseMm > 0 && Number.isFinite(baseMm)) {
    // Separación fija de las preferencias: es el paso mínimo; al alejar el zoom se usan múltiplos (1, 2, 5, 10...) para no saturar.
    const minor = fixedMinor(baseMm, zoom, minPx);
    return { minor, major: minor * 10 };
  }
  const steps = unit === 'in' ? STEPS_IN : STEPS;
  const minor = steps.find((s) => s * zoom >= minPx) ?? steps[steps.length - 1]!;
  return { minor, major: minor * 10 };
}

/** Texto de una cota de la regla (sin ceros de más), en mm o en pulgadas (decimales). */
export function formatTick(mm: number, unit: LengthUnit = 'mm'): string {
  if (unit === 'in') return formatLength(mm, 'in', { digits: 3, symbol: false, trim: true });
  return Number.isInteger(mm) ? String(mm) : String(Math.round(mm * 100) / 100);
}

/** px por mm al 100 %: tamaño real en una pantalla de 96 ppp (25,4 mm = 96 px). */
export const PX_PER_MM_AT_100 = 96 / 25.4;

export function zoomPercent(zoom: number): number {
  return Math.round((zoom / PX_PER_MM_AT_100) * 100);
}
