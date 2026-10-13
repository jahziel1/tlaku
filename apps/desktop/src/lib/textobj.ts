import type { Point } from '@/api/types';
import type { TextObject } from '@/bindings/TextObject';
import { polylineBez, type BezPath } from './bezpath';

/**
 * Ayudas del objeto de texto en la interfaz. El contorno real lo genera el motor (`outline`);
 * mientras esté vacío, la interfaz dibuja un marcador con el texto en una fuente del sistema y
 * una caja estimada para poder seleccionarlo y moverlo.
 */

/** Ancho medio de un carácter, en múltiplos del tamaño (estimación para fuentes proporcionales). */
const AVG_CHAR_EM = 0.55;
/** Parte del em que queda sobre la línea base y bajo ella. */
const ASCENT_EM = 0.8;
const DESCENT_EM = 0.25;

export function textLines(t: TextObject): string[] {
  return t.text.split(/\r?\n/);
}

/**
 * Caja local estimada del texto: la primera línea tiene su línea base en y = 0 y las siguientes
 * bajan `size · line_spacing`; el eje X arranca en 0 (izquierda), se centra en 0 o termina en 0
 * (derecha) según `align`.
 */
export function textPlaceholderBox(t: TextObject): [Point, Point] {
  const lines = textLines(t);
  const size = Math.max(t.size_mm, 0.1);
  const longest = Math.max(1, ...lines.map((l) => [...l].length));
  const width = Math.max(size * 0.5, longest * (size * AVG_CHAR_EM + t.letter_spacing_mm));
  const x0 = t.align === 'left' ? 0 : t.align === 'center' ? -width / 2 : -width;
  const bottom = -(lines.length - 1) * size * t.line_spacing - size * DESCENT_EM;
  return [[x0, bottom], [x0 + width, size * ASCENT_EM]];
}

export function textPlaceholderBez(t: TextObject): BezPath {
  const [[x0, y0], [x1, y1]] = textPlaceholderBox(t);
  return polylineBez([[x0, y0], [x1, y0], [x1, y1], [x0, y1]], true);
}

/** Fuente CSS para el marcador (la familia del objeto y, al final, una genérica del sistema). */
export function textFont(t: TextObject, sizePx: number): string {
  const family = t.font_family.trim() ? `${JSON.stringify(t.font_family)}, ` : '';
  return `${t.italic ? 'italic ' : ''}${t.bold ? 'bold ' : ''}${sizePx}px ${family}system-ui, sans-serif`;
}
