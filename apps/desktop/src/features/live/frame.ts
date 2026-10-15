import type { DeviceProfile, Point } from '@/api/types';
import { workToDoc, type DeviceFrame } from '@/lib/device';

/**
 * Del reporte de la máquina al documento. La vista en vivo ubica la posición REAL de la cabeza
 * sobre la ruta de la vista previa, que está en coordenadas del documento (mm de superficie, origen
 * abajo a la izquierda, Y hacia arriba). El reporte llega en coordenadas de trabajo de la máquina,
 * así que se deshace la transformación que el motor aplicó al generar el trabajo: origen del
 * perfil, intercambio X/Y y, con rotativo, el factor de Y.
 */

type RotaryFrame = Pick<DeviceProfile['rotary'], 'enabled' | 'kind' | 'mm_per_rotation' | 'roller_diameter' | 'object_diameter'>;

/**
 * mm de la controladora por mm de superficie en Y (1 sin rotativo). Copia el cálculo de
 * `rotary.rs::y_scale`: rodillo `mm_per_rotation / (π · roller_diameter)`, mandril
 * `mm_per_rotation / (π · object_diameter)`. `null` si los datos no sirven.
 */
export function rotaryYScale(device: { kind?: DeviceProfile['kind']; rotary?: RotaryFrame }): number | null {
  const r = device.rotary;
  if (!r?.enabled || device.kind === 'galvo') return 1;
  const diameter = r.kind === 'roller' ? r.roller_diameter : r.object_diameter;
  const k = r.mm_per_rotation / (Math.PI * diameter);
  return Number.isFinite(k) && k > 0 ? k : null;
}

/** El perfil reflejado de Y (`mirror`) cambia la Y del G-code dentro de la caja: no se puede deshacer aquí. */
function mirrored(device: { rotary?: { enabled?: boolean; mirror?: boolean } }): boolean {
  return device.rotary?.enabled === true && device.rotary.mirror === true;
}

/**
 * Posición de trabajo [x, y] de un reporte a coordenadas del documento. `null` si la
 * transformación no se puede deshacer (rotativo con datos inválidos o reflejado).
 */
export function reportToDoc(device: DeviceFrame & { kind?: DeviceProfile['kind'] }, work: readonly [number, number]): Point | null {
  const k = rotaryYScale(device);
  if (k === null || mirrored(device)) return null;
  // El factor de Y se aplicó al final, sobre la Y absoluta de la controladora.
  return workToDoc(device, [work[0], work[1] / k]);
}
