/**
 * Ajuste manual del fondo de la cámara fija (M35): la resolución (puntos por mm) y los
 * desplazamientos X/Y se aplican DESPUÉS de la alineación, sobre la matriz píxel -> mm. Sirven para
 * afinar a mano lo que la alineación no cuadró del todo; no la reemplazan ni se guardan en ella.
 *
 * Matemática pura. Con la cámara en el cabezal no se aplica (esa imagen sale de su propia
 * calibración).
 */
import type { CameraPrefs } from '@/api/types';
import { applyH, type Mat3, type Vec2 } from './geometry';

export interface BackdropAdjust {
  /** Puntos por mm fijados a mano; `null` = los de la alineación. */
  dpm: number | null;
  /** Desplazamiento del fondo (mm de documento). */
  dx: number;
  dy: number;
}

export const NO_ADJUST: BackdropAdjust = { dpm: null, dx: 0, dy: 0 };
/** Límites de los controles (el motor aplica los mismos al guardar). */
export const MAX_OFFSET_MM = 1000;
export const MAX_DPM = 10_000;

export function adjustOf(prefs: Pick<CameraPrefs, 'dpm' | 'offset_x_mm' | 'offset_y_mm'>): BackdropAdjust {
  const ok = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
  return {
    dpm: ok(prefs.dpm) && prefs.dpm > 0 ? prefs.dpm : null,
    dx: ok(prefs.offset_x_mm) ? prefs.offset_x_mm : 0,
    dy: ok(prefs.offset_y_mm) ? prefs.offset_y_mm : 0,
  };
}

export const isAdjusted = (a: BackdropAdjust): boolean => a.dpm !== null || a.dx !== 0 || a.dy !== 0;

/**
 * Puntos por mm (píxeles por mm) de la imagen en su centro: la raíz de la razón entre el área en
 * píxeles y en mm de un cuadrito del centro (vale también con perspectiva). 0 si no se puede medir.
 */
export function autoDpm(matrix: Mat3, space: readonly [number, number]): number {
  const [cx, cy] = [space[0] / 2, space[1] / 2];
  const [x0, x1] = [applyH(matrix, [cx - 0.5, cy]), applyH(matrix, [cx + 0.5, cy])];
  const [y0, y1] = [applyH(matrix, [cx, cy - 0.5]), applyH(matrix, [cx, cy + 0.5])];
  const det = (x1[0] - x0[0]) * (y1[1] - y0[1]) - (y1[0] - y0[0]) * (x1[1] - x0[1]);
  const area = Math.abs(det);
  return Number.isFinite(area) && area > 1e-12 ? 1 / Math.sqrt(area) : 0;
}

/**
 * Matriz píxel -> mm con el ajuste. La escala (`auto / dpm`) se hace alrededor del punto de la
 * cama que ve el centro de la imagen, y el desplazamiento se suma después.
 */
export function adjustMatrix(matrix: Mat3, space: readonly [number, number], adj: BackdropAdjust): Mat3 {
  if (!isAdjusted(adj)) return matrix;
  let s = 1;
  if (adj.dpm !== null) {
    const auto = autoDpm(matrix, space);
    if (auto > 0) s = auto / adj.dpm;
  }
  const c: Vec2 = applyH(matrix, [space[0] / 2, space[1] / 2]);
  // T(dx, dy) · (escala s alrededor de c): en la tercera columna entra c·(1 - s) + d.
  const ax = c[0] * (1 - s) + adj.dx;
  const ay = c[1] * (1 - s) + adj.dy;
  const rows: Mat3 = [
    [s * matrix[0][0] + ax * matrix[2][0], s * matrix[0][1] + ax * matrix[2][1], s * matrix[0][2] + ax * matrix[2][2]],
    [s * matrix[1][0] + ay * matrix[2][0], s * matrix[1][1] + ay * matrix[2][1], s * matrix[1][2] + ay * matrix[2][2]],
    [matrix[2][0], matrix[2][1], matrix[2][2]],
  ];
  return rows;
}
