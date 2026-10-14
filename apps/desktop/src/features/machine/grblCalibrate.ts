/**
 * Calibración de ejes de GRBL (M25): `$100`, `$101` y `$102` son pasos por mm de X, Y y Z. Se manda
 * mover el eje una distancia conocida, se mide lo que se movió de verdad y el valor nuevo es
 * `actual × pedido ÷ medido` (si recorrió menos de lo pedido, hacen falta MÁS pasos por mm).
 */
export const GRBL_AXES = [
  { id: 100, axis: 'X' },
  { id: 101, axis: 'Y' },
  { id: 102, axis: 'Z' },
] as const;

/** Una corrección razonable no pasa de ±25 %: más es casi seguro un error de medida. */
const MIN_RATIO = 0.8;
const MAX_RATIO = 1.25;
const MAX_MM = 10_000;

export type GrblCalibration =
  | { ok: true; current: number; proposed: number; ratio: number }
  | { ok: false; reason: 'commanded' | 'measured' | 'current' | 'ratio' | 'same'; ratio?: number };

/** Redondea a 3 decimales (los pasos por mm de GRBL admiten decimales). */
const round3 = (v: number) => Math.round(v * 1000) / 1000;

export function calibrateGrblSteps(current: number, commanded: number, measured: number): GrblCalibration {
  const valid = (v: number) => Number.isFinite(v) && v > 0 && v <= MAX_MM;
  if (!valid(commanded)) return { ok: false, reason: 'commanded' };
  if (!valid(measured)) return { ok: false, reason: 'measured' };
  if (!Number.isFinite(current) || current <= 0) return { ok: false, reason: 'current' };
  const ratio = measured / commanded;
  if (ratio < MIN_RATIO || ratio > MAX_RATIO) return { ok: false, reason: 'ratio', ratio };
  const proposed = round3((current * commanded) / measured);
  if (proposed === round3(current)) return { ok: false, reason: 'same', ratio };
  return { ok: true, current, proposed, ratio };
}
