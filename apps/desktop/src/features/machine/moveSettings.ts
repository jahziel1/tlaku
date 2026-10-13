import { readPref, writePref } from '@/lib/platform';
import { JOG_STEPS } from './jogMath';

// ---------------------------------------------------------------------------
// Posición final
// ---------------------------------------------------------------------------

/** Qué hace el cabezal al terminar un trabajo: quedarse, volver al origen o ir a un punto. */
export type FinishMode = 'stay' | 'origin' | 'point';

export interface FinishPosition {
  mode: FinishMode;
  /** Punto de destino con `mode: 'point'`, en mm del documento (origen abajo a la izquierda, Y arriba). */
  x: number;
  y: number;
}

export const FINISH_MODES: readonly FinishMode[] = ['stay', 'origin', 'point'];
export const DEFAULT_FINISH: FinishPosition = { mode: 'stay', x: 0, y: 0 };

const MAX_COORD = 100000;
const num = (v: unknown, fallback: number): number =>
  typeof v === 'number' && Number.isFinite(v) ? Math.min(MAX_COORD, Math.max(-MAX_COORD, Math.round(v * 1000) / 1000)) : fallback;

/** Posición final válida a partir de datos de cualquier origen. */
export function sanitizeFinish(raw: unknown): FinishPosition {
  const src = raw && typeof raw === 'object' ? (raw as Record<string, unknown>) : {};
  const mode = FINISH_MODES.find((m) => m === src.mode) ?? DEFAULT_FINISH.mode;
  return { mode, x: num(src.x, DEFAULT_FINISH.x), y: num(src.y, DEFAULT_FINISH.y) };
}

export function readFinish(): FinishPosition {
  try {
    return sanitizeFinish(JSON.parse(readPref('finishPosition') ?? 'null'));
  } catch {
    return { ...DEFAULT_FINISH };
  }
}

export function writeFinish(f: FinishPosition): void {
  writePref('finishPosition', JSON.stringify(f));
}

// ---------------------------------------------------------------------------
// Pasos del jog
// ---------------------------------------------------------------------------

export const MAX_JOG_STEPS = 6;
export const MAX_JOG_STEP_MM = 500;
export const MIN_JOG_STEP_MM = 0.01;

/** Pasos válidos: positivos, entre 0,01 y 500 mm, sin repetir, de menor a mayor y a lo más seis. */
export function sanitizeJogSteps(raw: unknown): number[] {
  const list = Array.isArray(raw) ? raw : [];
  const steps = list
    .filter((v): v is number => typeof v === 'number' && Number.isFinite(v))
    .map((v) => Math.round(Math.min(MAX_JOG_STEP_MM, Math.max(MIN_JOG_STEP_MM, v)) * 1000) / 1000);
  const unique = [...new Set(steps)].sort((a, b) => a - b).slice(0, MAX_JOG_STEPS);
  return unique.length > 0 ? unique : [...JOG_STEPS];
}

/**
 * Lee los pasos de un texto: separados por «;» o espacios («0,1; 1; 10» o «0.1 1 10»), o por comas
 * si no hay otro separador («0.1,1,10»). Devuelve los pasos saneados, o `null` si algún valor no es
 * un número positivo o no hay ninguno.
 */
export function parseJogSteps(text: string): number[] | null {
  const trimmed = text.trim();
  // Con «;» o espacios, la coma es decimal («0,1; 1») o sobra al final del número («0.1, 1»).
  const parts = /[;\s]/.test(trimmed) ? trimmed.split(/[;\s]+/) : trimmed.split(',');
  const tokens = parts.map((x) => x.trim().replace(/,+$/, '').replace(',', '.')).filter(Boolean);
  if (tokens.length === 0) return null;
  const values = tokens.map(Number);
  if (values.some((v) => !Number.isFinite(v) || v <= 0)) return null;
  return sanitizeJogSteps(values);
}

export function readJogSteps(): number[] {
  try {
    return sanitizeJogSteps(JSON.parse(readPref('jogSteps') ?? 'null'));
  } catch {
    return [...JOG_STEPS];
  }
}

export function writeJogSteps(steps: readonly number[]): void {
  writePref('jogSteps', JSON.stringify(steps));
}
