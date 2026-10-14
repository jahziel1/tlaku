import { getApi } from '@/api';
import type { Point } from '@/api/types';
import type { DesignMark } from './marks';
import type { FitChoice, FitResult, MarkPair } from './types';

/** Lo que el usuario elige para el ajuste: un modo concreto o «según cuántas marcas haya». */
export type ModeChoice = FitChoice;

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

/**
 * Ajuste de las marcas medidas, o `null` si todavía falta alguna. El cálculo es del motor (comando
 * `fit_registration`): él también elige el modo según las marcas cuando `choice` es `auto` o la
 * afín no se puede resolver.
 */
export async function computeRegistration(marks: readonly DesignMark[], physical: readonly (Point | null)[], choice: ModeChoice): Promise<FitResult | null> {
  const pairs = buildPairs(marks, physical);
  if (!pairs) return null;
  return getApi().fitRegistration({ pairs, mode: choice });
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
