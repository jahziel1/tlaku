import type { JobArgs } from '@/api/types';

/**
 * Lo último que `startJob` mandó al motor. La vista en vivo lo toma cuando llega el evento
 * `started`: así sabe con qué argumentos se calculó el trabajo que corre (el asistente de tiling
 * los ajusta, y con «desde la posición actual» la vista previa en caché podría no coincidir).
 * Nada se envía a la máquina desde aquí.
 */
export interface JobStart {
  args: JobArgs;
  /** El llamador ajustó los argumentos (tiling): la vista previa en caché no es la del trabajo. */
  tweaked: boolean;
  atMs: number;
}

/** Cuánto vale un inicio anotado (ms): pasado eso, un `started` que llegue es de otro origen (reanudación). */
export const START_NOTE_TTL_MS = 60_000;

let pending: JobStart | null = null;

export function noteJobStart(args: JobArgs, tweaked: boolean, nowMs: number = Date.now()): void {
  pending = { args, tweaked, atMs: nowMs };
}

/** Toma (y borra) el inicio anotado, si es reciente. */
export function takeJobStart(nowMs: number = Date.now()): JobStart | null {
  const p = pending;
  pending = null;
  return p && nowMs - p.atMs <= START_NOTE_TTL_MS ? p : null;
}

export function clearJobStart(): void {
  pending = null;
}
