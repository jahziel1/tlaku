/**
 * Qué imágenes pedir al motor para la vista «por tonos»: una de toda la hoja de material (se pide
 * siempre) y, al acercar el zoom, otra solo de lo que se ve con un píxel de imagen por píxel de
 * pantalla, para que se vean los puntos del tramado. Lógica pura.
 */
import type { ViewTransform } from '@/lib/viewport';
import type { Box } from './types';

/** Lado mayor (px) de la imagen de toda la hoja. */
export const FULL_MAX_PX = 1600;
/** Lado mayor (px) máximo de una imagen de detalle. */
export const DETAIL_MAX_PX = 2400;
export const DETAIL_MIN_PX = 128;
/** Cuánto se ensancha la zona de detalle a cada lado (fracción) para que moverse un poco no la invalide. */
export const DETAIL_PAD = 0.2;
/** Tolerancia: una imagen sirve si su píxel no es más de esta vez más grueso que el de la pantalla. */
const COARSE_TOLERANCE = 1.25;

/** Zona del documento (mm, Y hacia arriba) que muestra un lienzo de `width` x `height` px CSS. */
export function visibleBox(view: ViewTransform, width: number, height: number): Box {
  return [(0 - view.panX) / view.zoom, (view.panY - height) / view.zoom, (width - view.panX) / view.zoom, view.panY / view.zoom];
}

export function intersect(a: Box, b: Box): Box | null {
  const r: Box = [Math.max(a[0], b[0]), Math.max(a[1], b[1]), Math.min(a[2], b[2]), Math.min(a[3], b[3])];
  return r[2] > r[0] && r[3] > r[1] ? r : null;
}

export function contains(outer: Box, inner: Box, slack = 1e-6): boolean {
  return outer[0] <= inner[0] + slack && outer[1] <= inner[1] + slack && outer[2] >= inner[2] - slack && outer[3] >= inner[3] - slack;
}

export interface DetailPlanInput {
  view: ViewTransform;
  /** Tamaño del lienzo en px CSS y razón de píxeles del dispositivo. */
  width: number;
  height: number;
  dpr: number;
  sheet: Box;
  /** Píxel (mm) de la imagen de toda la hoja. */
  fullPxMm: number;
  /** La imagen de detalle que ya hay, si hay. */
  detail: { covered: Box; pxMm: number } | null;
}

export type DetailPlan = { kind: 'keep' } | { kind: 'drop' } | { kind: 'request'; region: Box; maxPx: number };

/** Decide si hace falta pedir (o soltar) una imagen de detalle para la vista de ahora. */
export function planDetail(i: DetailPlanInput): DetailPlan {
  const wanted = 1 / Math.max(1e-9, i.view.zoom * i.dpr); // mm por píxel de pantalla
  const visible = intersect(visibleBox(i.view, i.width, i.height), i.sheet);
  if (!visible) return { kind: i.detail ? 'drop' : 'keep' };
  // La imagen general ya es tan fina como la pantalla: no hace falta más.
  if (i.fullPxMm <= wanted * COARSE_TOLERANCE) return { kind: i.detail ? 'drop' : 'keep' };
  const d = i.detail;
  if (d && contains(d.covered, visible) && d.pxMm <= wanted * COARSE_TOLERANCE && d.pxMm >= wanted / 4) return { kind: 'keep' };
  const w = visible[2] - visible[0];
  const h = visible[3] - visible[1];
  const padded: Box = [visible[0] - w * DETAIL_PAD, visible[1] - h * DETAIL_PAD, visible[2] + w * DETAIL_PAD, visible[3] + h * DETAIL_PAD];
  const region = intersect(padded, i.sheet) ?? visible;
  const side = Math.max(region[2] - region[0], region[3] - region[1]);
  const maxPx = Math.min(DETAIL_MAX_PX, Math.max(DETAIL_MIN_PX, Math.ceil(side / wanted)));
  return { kind: 'request', region, maxPx };
}
