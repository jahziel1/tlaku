import type { Point } from '@/api/types';
import type { Bounds } from '@/lib/geometry';
import { worldToScreen, type ViewTransform } from '@/lib/viewport';

export interface Box {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export const PANEL_WIDTH = 300;
export const PANEL_MARGIN = 8;
/** Alto aproximado del panel, para saber si llega a tapar la selección. */
export const PANEL_EST_HEIGHT = 460;
/** A la izquierda el panel baja para no tapar el nombre del dispositivo. */
export const PANEL_LEFT_TOP = 56;

/** Caja de pantalla (px del área del lienzo) de unos límites en mm. */
export function screenBox(view: ViewTransform, b: Bounds): Box {
  const [a, c] = [worldToScreen(view, b[0]), worldToScreen(view, b[1])] as [Point, Point];
  return { left: Math.min(a[0], c[0]), right: Math.max(a[0], c[0]), top: Math.min(a[1], c[1]), bottom: Math.max(a[1], c[1]) };
}

const overlaps = (a: Box, b: Box) => a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top;

/**
 * Lado del lienzo donde flota el panel sin tapar la selección: a la derecha por defecto y a la
 * izquierda si la selección llega hasta allí. Si no cabe ninguno, se queda a la derecha.
 */
export function choosePanelSide(selection: Box | null, viewportW: number, panelW = PANEL_WIDTH): 'left' | 'right' {
  if (!selection) return 'right';
  const w = Math.min(panelW, Math.max(0, viewportW - PANEL_MARGIN * 2));
  const top = PANEL_MARGIN;
  const bottom = PANEL_MARGIN + PANEL_EST_HEIGHT;
  const right: Box = { left: viewportW - PANEL_MARGIN - w, right: viewportW - PANEL_MARGIN, top, bottom };
  const left: Box = { left: PANEL_MARGIN, right: PANEL_MARGIN + w, top: PANEL_LEFT_TOP, bottom: PANEL_LEFT_TOP + PANEL_EST_HEIGHT };
  if (!overlaps(selection, right)) return 'right';
  if (!overlaps(selection, left)) return 'left';
  return 'right';
}
