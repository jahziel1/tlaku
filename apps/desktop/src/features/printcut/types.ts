import type { Affine, Point } from '@/api/types';

/**
 * Tipos del ajuste de Print & Cut. El cálculo es del motor (`tlaku_pipeline::fit_registration`,
 * comando `fit_registration`); estos son los de su respuesta, con los mismos nombres de campo.
 */

/** Desviación de la escala respecto de 1 (fracción) a partir de la cual el motor avisa: 0,5 %. */
export const SCALE_WARN = 0.005;
/** Distancia mínima (mm) entre dos marcas: más cerca no se puede medir el giro. */
export const MIN_SEPARATION_MM = 1;

/** Transformación que se calcula. */
export type FitMode = 'rigid' | 'similarity' | 'affine';
/** Lo que se pide: un modo concreto o «según cuántas marcas haya». */
export type FitChoice = FitMode | 'auto';

export interface MarkPair {
  design: Point;
  physical: Point;
}

export type FitWarning =
  | { kind: 'mirrored' }
  | { kind: 'scale'; pct: number }
  | { kind: 'anisotropy'; xPct: number; yPct: number };

export type FitError =
  | { kind: 'few'; need: number; have: number }
  | { kind: 'nan' }
  | { kind: 'close'; a: number; b: number }
  | { kind: 'collinear' };

export interface RegistrationFit {
  transform: Affine;
  /** El modo que se usó de verdad (puede diferir del pedido). */
  mode: FitMode;
  /** Giro en grados, antihorario. */
  rotationDeg: number;
  scale: number;
  scaleXY: [number, number];
  /** Error de cada marca tras transformar, mm. */
  residuals: number[];
  residualRms: number;
  residualMax: number;
  warnings: FitWarning[];
}

export type FitResult = { ok: true; fit: RegistrationFit } | { ok: false; error: FitError };
