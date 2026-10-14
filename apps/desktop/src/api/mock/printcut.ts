import type { Affine, Point } from '@/api/types';

/**
 * Hoja impresa simulada para probar Print & Cut con `npm run dev` (sin máquina ni papel): la
 * impresión quedó corrida 3,2 mm a la derecha, 1,5 mm arriba y girada 0,8° respecto de lo que dice
 * el diseño. Es la «verdad» que el asistente debe recuperar al medir las marcas.
 */
export const MOCK_SHEET_SHIFT_MM: Point = [3.2, 1.5];
export const MOCK_SHEET_ROTATION_DEG = 0.8;

/** Del diseño a donde cayó en la hoja: gira alrededor del origen de la cama y luego traslada. */
export function mockSheetTransform(shift: Point = MOCK_SHEET_SHIFT_MM, rotationDeg = MOCK_SHEET_ROTATION_DEG): Affine {
  const r = (rotationDeg * Math.PI) / 180;
  const c = Math.cos(r);
  const s = Math.sin(r);
  return [c, s, -s, c, shift[0], shift[1]];
}

/** Dónde está, en la cama, la marca que el diseño pone en `designPoint`. */
export function mockSheetPoint(designPoint: Point, sheet: Affine = mockSheetTransform()): Point {
  return [sheet[0] * designPoint[0] + sheet[2] * designPoint[1] + sheet[4], sheet[1] * designPoint[0] + sheet[3] * designPoint[1] + sheet[5]];
}
