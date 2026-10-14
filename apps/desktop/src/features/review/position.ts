/** Dónde flota «Revisar diseño»: debajo de la barra de pestañas del panel derecho, y movible. */

export interface PanelPos {
  x: number;
  y: number;
}

/** Ancho del panel (px) y márgenes con la ventana. Los mismos que usa `review.css`. */
export const PANEL_W = 420;
const MARGIN = 16;
/** Alto de la barra superior y de la barra de estado (px): el panel no se mete debajo de ellas. */
const TOPBAR_H = 44;
const STATUS_H = 26;
/** Lo mínimo del panel (px de alto) que debe quedar a la vista para poder volver a agarrarlo. */
const MIN_VISIBLE_H = 64;

export interface WindowSize {
  w: number;
  h: number;
}

/** Ancho real del panel en una ventana de ese ancho. */
export const panelWidth = (win: WindowSize): number => Math.min(PANEL_W, win.w - 2 * MARGIN);

/** Mantiene el panel dentro de la ventana, entre la barra superior y la de estado. */
export function clampPos(p: PanelPos, win: WindowSize): PanelPos {
  const maxX = Math.max(0, win.w - panelWidth(win));
  const maxY = Math.max(TOPBAR_H, win.h - STATUS_H - MIN_VISIBLE_H);
  return { x: Math.min(maxX, Math.max(0, p.x)), y: Math.min(maxY, Math.max(TOPBAR_H, p.y)) };
}

/**
 * Posición inicial: pegada a la derecha y justo debajo de la barra de pestañas (`tabsBottom`, el borde
 * de abajo de la barra, en px de ventana). Sin barra visible, queda bajo la barra superior.
 */
export function defaultPos(win: WindowSize, tabsBottom: number | null): PanelPos {
  const y = (tabsBottom ?? TOPBAR_H + MARGIN) + 8;
  return clampPos({ x: win.w - panelWidth(win) - MARGIN, y }, win);
}

/** Borde de abajo de la barra de pestañas del panel derecho, o null si no está en pantalla. */
export function tabsBottom(): number | null {
  if (typeof document === 'undefined') return null;
  const bar = document.querySelector<HTMLElement>('.dock [role="tablist"]');
  if (!bar) return null;
  const r = bar.getBoundingClientRect();
  return r.width === 0 && r.height === 0 ? null : r.bottom;
}

/** Tamaño de la ventana de la app (un valor razonable fuera del navegador). */
export function windowSize(): WindowSize {
  return typeof window === 'undefined' ? { w: 1440, h: 900 } : { w: window.innerWidth, h: window.innerHeight };
}
