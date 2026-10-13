/** Vista (zoom y paneo) de la vista previa, en unidades de la imagen (mm). */
export interface TraceView {
  /** 1 = imagen completa. */
  zoom: number;
  /** Centro de la vista en mm (Y hacia abajo, como el SVG). */
  cx: number;
  cy: number;
}

export const MIN_ZOOM = 1;
export const MAX_ZOOM = 40;

export const fitView = (w: number, h: number): TraceView => ({ zoom: 1, cx: w / 2, cy: h / 2 });

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/** Mantiene la vista dentro de la imagen: el centro no sale de ella. */
export function clampView(v: TraceView, w: number, h: number): TraceView {
  const zoom = clamp(v.zoom, MIN_ZOOM, MAX_ZOOM);
  const halfW = w / zoom / 2;
  const halfH = h / zoom / 2;
  return { zoom, cx: clamp(v.cx, halfW, w - halfW), cy: clamp(v.cy, halfH, h - halfH) };
}

/** `viewBox` del SVG para la vista. */
export function viewBoxOf(v: TraceView, w: number, h: number): string {
  const vw = w / v.zoom;
  const vh = h / v.zoom;
  return `${v.cx - vw / 2} ${v.cy - vh / 2} ${vw} ${vh}`;
}

/** Acerca o aleja por `factor` dejando fijo el punto (px, py) de la imagen bajo el cursor. */
export function zoomAt(v: TraceView, factor: number, px: number, py: number, w: number, h: number): TraceView {
  const zoom = clamp(v.zoom * factor, MIN_ZOOM, MAX_ZOOM);
  const k = v.zoom / zoom;
  return clampView({ zoom, cx: px - (px - v.cx) * k, cy: py - (py - v.cy) * k }, w, h);
}

/** Desplaza la vista `dx`, `dy` unidades de la imagen. */
export function panBy(v: TraceView, dx: number, dy: number, w: number, h: number): TraceView {
  return clampView({ ...v, cx: v.cx + dx, cy: v.cy + dy }, w, h);
}
