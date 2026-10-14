import type { Point } from '@/api/types';
import { areCollinear, autoMode, fitRegistration, type FitMode, type FitResult, type MarkPair } from './fit';
import type { DesignMark } from './marks';

/** Lo que el usuario elige para el ajuste: un modo concreto o «según cuántas marcas haya». */
export type ModeChoice = FitMode | 'auto';

/**
 * Modo que se usa de verdad. Con 3 o más marcas en línea recta la afín no se puede resolver: se
 * cae a la similitud. Pedir `affine` con 2 marcas también cae a la similitud.
 */
export function resolveMode(choice: ModeChoice, designPoints: readonly Point[]): FitMode {
  const wanted = choice === 'auto' ? autoMode(designPoints.length) : choice;
  if (wanted === 'affine' && (designPoints.length < 3 || areCollinear(designPoints))) return 'similarity';
  return wanted;
}

/** Pares (diseño, físico) si ya se midieron todas las marcas; `null` si falta alguna. */
export function buildPairs(marks: readonly DesignMark[], physical: readonly (Point | null)[]): MarkPair[] | null {
  if (marks.length === 0 || physical.length < marks.length) return null;
  const pairs: MarkPair[] = [];
  for (let i = 0; i < marks.length; i++) {
    const p = physical[i];
    if (!p) return null;
    pairs.push({ design: marks[i]!.point, physical: p });
  }
  return pairs;
}

/** Ajuste de las marcas medidas, o `null` si todavía falta alguna. */
export function computeRegistration(marks: readonly DesignMark[], physical: readonly (Point | null)[], choice: ModeChoice): FitResult | null {
  const pairs = buildPairs(marks, physical);
  if (!pairs) return null;
  return fitRegistration(pairs, resolveMode(choice, marks.map((m) => m.point)));
}

/** La siguiente marca sin medir después de `from` (dando la vuelta); `from` si ya están todas. */
export function nextMissing(physical: readonly (Point | null)[], from: number): number {
  const n = physical.length;
  for (let k = 1; k <= n; k++) {
    const i = (from + k) % n;
    if (!physical[i]) return i;
  }
  return from;
}

/** Error residual máximo (mm) que se da por bueno sin avisar: una marca bien puesta cae muy por debajo. */
export const GOOD_RESIDUAL_MM = 0.3;

export function isGoodFit(residualMaxMm: number): boolean {
  return residualMaxMm <= GOOD_RESIDUAL_MM;
}
