/** Ancho y alto aproximados del cartel junto al puntero, para voltearlo o apartarlo cuando no cabe. */
export const TIP_W = 200;
export const TIP_H = 30;

/** Rectángulo en píxeles del lienzo (origen arriba a la izquierda). */
export interface Box {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/**
 * Posición vertical del cartel que va en (x, y): si taparía la barra de la herramienta (que flota sobre
 * el lienzo y se pinta encima del cartel), se baja justo debajo de ella; si no, queda donde estaba.
 * El cartel se voltea a la izquierda del puntero cuando no cabe a la derecha (ver `CanvasTip`).
 */
export function tipYClearOf(x: number, y: number, viewportWidth: number, bar: Box | null): number {
  if (!bar) return y;
  const left = x + TIP_W > viewportWidth ? x - TIP_W - 32 : x;
  const right = left + TIP_W;
  const overlapsX = right > bar.left && left < bar.right;
  const overlapsY = y + TIP_H > bar.top && y < bar.bottom;
  return overlapsX && overlapsY ? bar.bottom + 6 : y;
}

/** Caja de una barra de herramienta (`.tool-bar`) dentro del lienzo `el`, o null si no está a la vista. */
export function toolBarBox(el: HTMLElement): Box | null {
  // Las pruebas de herramientas pasan un lienzo de mentira sin DOM.
  if (typeof el.querySelector !== 'function') return null;
  const bar = el.querySelector<HTMLElement>('.tool-bar');
  if (!bar) return null;
  const a = el.getBoundingClientRect();
  const b = bar.getBoundingClientRect();
  if (b.width === 0 && b.height === 0) return null;
  return { left: b.left - a.left, top: b.top - a.top, right: b.right - a.left, bottom: b.bottom - a.top };
}
