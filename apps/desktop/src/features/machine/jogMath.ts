import type { DeviceProfile, Point } from '@/api/types';
import { docDeltaToWork, workToDoc } from '@/lib/device';

/** Pasos del jog en mm. */
export const JOG_STEPS = [0.1, 1, 10, 50] as const;
export const DEFAULT_JOG_STEP = 10;
export const DEFAULT_JOG_SPEED = 3000;
/** Tiempo que hay que mantener pulsado para pasar de un paso a jog continuo, en ms. */
export const HOLD_MS = 350;
/** Un jog continuo con menos recorrido que esto no se envía. */
export const MIN_CONTINUOUS_MM = 0.5;

/** Dirección en el plano del documento: X a la derecha, Y arriba. */
export interface JogDir {
  x: -1 | 0 | 1;
  y: -1 | 0 | 1;
}

/** Las 8 direcciones del pad con su clave estable (para etiquetas y pruebas). */
export const JOG_DIRS = {
  upLeft: { x: -1, y: 1 },
  up: { x: 0, y: 1 },
  upRight: { x: 1, y: 1 },
  left: { x: -1, y: 0 },
  right: { x: 1, y: 0 },
  downLeft: { x: -1, y: -1 },
  down: { x: 0, y: -1 },
  downRight: { x: 1, y: -1 },
} as const satisfies Record<string, JogDir>;

export type JogDirName = keyof typeof JOG_DIRS;

/** Flechas del teclado: sin diagonales (se combinan con el ratón). */
export function dirFromKey(key: string): JogDir | null {
  switch (key) {
    case 'ArrowLeft':
      return JOG_DIRS.left;
    case 'ArrowRight':
      return JOG_DIRS.right;
    case 'ArrowUp':
      return JOG_DIRS.up;
    case 'ArrowDown':
      return JOG_DIRS.down;
    default:
      return null;
  }
}

const round3 = (n: number) => Math.round(n * 1000) / 1000;

/** Paso efectivo: Mayús lo multiplica por 10 y Alt lo divide por 10 (como las flechas del editor). */
export function modifiedStep(step: number, mods: { shift?: boolean; alt?: boolean }): number {
  if (mods.shift && !mods.alt) return round3(step * 10);
  if (mods.alt && !mods.shift) return round3(step / 10);
  return step;
}

/** Desplazamiento relativo de G-code para un paso en la dirección `dir` (según el origen del perfil). */
export function stepDelta(device: Pick<DeviceProfile, 'origin'>, dir: JogDir, step: number): Point {
  const [dx, dy] = docDeltaToWork(device, dir.x * step, dir.y * step);
  return [round3(dx) + 0, round3(dy) + 0]; // `+ 0` evita el -0
}

/**
 * Recorrido de un jog continuo: desde la posición actual hasta el borde de la cama en esa dirección
 * (así un jog que no se cancele nunca sale de la cama). Devuelve null si no se conoce la posición
 * o no queda recorrido. `work` es la posición de trabajo de la máquina.
 */
export function continuousDelta(device: Pick<DeviceProfile, 'bed' | 'origin'>, dir: JogDir, work: Point | null): Point | null {
  if (!work) return null;
  const [hx, hy] = workToDoc(device, work);
  const room = (u: number, pos: number, size: number) => (u > 0 ? size - pos : u < 0 ? pos : Infinity);
  const rx = room(dir.x, hx, device.bed[0]);
  const ry = room(dir.y, hy, device.bed[1]);
  // En diagonal se recorre lo mismo en ambos ejes hasta el primer borde.
  const dist = Math.min(rx, ry);
  if (!Number.isFinite(dist) || dist < MIN_CONTINUOUS_MM) return null;
  return stepDelta(device, dir, dist);
}

/** Velocidad de jog limitada a lo que soporta la máquina (la menor de sus velocidades máximas). */
export function clampJogSpeed(speed: number, device: Pick<DeviceProfile, 'max_rate'>): number {
  const cap = Math.min(device.max_rate[0], device.max_rate[1]);
  if (!Number.isFinite(speed) || speed <= 0) return DEFAULT_JOG_SPEED;
  return Math.max(1, Math.min(cap, Math.round(speed)));
}
