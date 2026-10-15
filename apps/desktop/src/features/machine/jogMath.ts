import type { DeviceProfile, Point } from '@/api/types';
import { t } from '@/i18n';
import { docDeltaToWork, workToDoc, type DeviceFrame } from '@/lib/device';
import { fmt } from '@/lib/platform';

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
export function stepDelta(device: Pick<DeviceProfile, 'origin'> & Partial<Pick<DeviceProfile, 'swap_xy' | 'rotary'>>, dir: JogDir, step: number): Point {
  const [dx, dy] = docDeltaToWork(device, dir.x * step, dir.y * step);
  return [round3(dx) + 0, round3(dy) + 0]; // `+ 0` evita el -0
}

/**
 * Recorrido de un jog continuo: desde la posición actual hasta el borde de la cama en esa dirección
 * (así un jog que no se cancele nunca sale de la cama). Devuelve null si no se conoce la posición
 * o no queda recorrido. `work` es la posición de trabajo de la máquina.
 */
export function continuousDelta(device: DeviceFrame, dir: JogDir, work: Point | null): Point | null {
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

/** Cuánto se tolera pasar del borde de la cama (igual que en el motor). */
export const BED_TOLERANCE_MM = 0.5;

/** Un eje que se saldría de la cama: a dónde llegaría (mm del documento) y el largo de la cama en ese eje. */
export interface OutsideAxis {
  axis: 'X' | 'Y';
  value: number;
  max: number;
}

/**
 * Ejes por los que un jog relativo `delta` (en ejes de la máquina) se saldría de la cama, o `[]` si
 * cabe o no se conoce la posición. Mismo marco que `continuousDelta`: la posición de trabajo sobre
 * la cama del perfil, con 0,5 mm de margen. El motor repite la comprobación con la MPos real.
 */
export function jogOutsideBed(device: DeviceFrame, work: Point | null, delta: Point): OutsideAxis[] {
  if (!work || device.rotary?.enabled) return [];
  const to = workToDoc(device, [work[0] + delta[0], work[1] + delta[1]]);
  return pointOutsideBed(device, to);
}

/** Ejes por los que el punto del documento `p` queda fuera de la cama (con el margen de 0,5 mm). */
export function pointOutsideBed(device: Pick<DeviceProfile, 'bed'>, p: Point): OutsideAxis[] {
  const out: OutsideAxis[] = [];
  (['X', 'Y'] as const).forEach((axis, i) => {
    const max = device.bed[i]!;
    if (p[i]! < -BED_TOLERANCE_MM || p[i]! > max + BED_TOLERANCE_MM) out.push({ axis, value: p[i]!, max });
  });
  return out;
}

/** «El movimiento saldría de la cama: X llegaría a -4.97 mm (límite 0 a 400)». No recorta: solo dice el motivo. */
export function outsideBedMessage(out: readonly OutsideAxis[]): string {
  const detail = out.map((o) => t('machine.jog.outsideAxis', { axis: o.axis, value: fmt(o.value, 2), max: fmt(o.max, 0) })).join('; ');
  return t('machine.jog.outsideBed', { detail });
}

/** Velocidad de jog limitada a lo que soporta la máquina (la menor de sus velocidades máximas). */
export function clampJogSpeed(speed: number, device: Pick<DeviceProfile, 'max_rate'>): number {
  const cap = Math.min(device.max_rate[0], device.max_rate[1]);
  if (!Number.isFinite(speed) || speed <= 0) return DEFAULT_JOG_SPEED;
  return Math.max(1, Math.min(cap, Math.round(speed)));
}
