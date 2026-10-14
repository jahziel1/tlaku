import type { DeviceProfile, GalvoSafetyAck, LaserSource } from '@/api/types';
import { selectActiveDevice } from '@/store/selectors';
import type { AppState } from '@/store/types';
import { baseVirtualPort } from './virtualPort';

/**
 * Galvo con tarjeta JCZ/BJJCZ (EXPERIMENTAL). No usa puerto serie: el motor habla con la tarjeta por
 * USB (9588:9899). La lista de puertos trae una entrada por dispositivo BJJCZ (`usb:galvo:<n>`) y la
 * tarjeta simulada (`virtual:galvo`). Solo se ofrecen con un perfil galvo.
 */
export const GALVO_VIRTUAL = 'virtual:galvo';
export const GALVO_USB_PREFIX = 'usb:galvo:';
/** VID:PID de la tarjeta compatible (LMCV4-FIBER-M). */
export const GALVO_VID_PID = '9588:9899';

/** Texto que el motor deja en el mensaje de un trabajo cuando detener NO se confirmó (`UNCONFIRMED_MARK`). */
export const GALVO_UNCONFIRMED = 'NO hubo confirmación';

/** ¿El perfil activo es una máquina galvo? */
export function selectIsGalvo(s: Pick<AppState, 'devices' | 'activeDeviceId'>): boolean {
  return selectActiveDevice(s).kind === 'galvo';
}

/** ¿El perfil es galvo? Lo que sigue falla con los demás tipos. */
export function isGalvoDevice(device: Pick<DeviceProfile, 'kind'>): boolean {
  return device.kind === 'galvo';
}

export function isGalvoVirtualPort(port: string | null | undefined): boolean {
  return !!port && baseVirtualPort(port) === GALVO_VIRTUAL;
}

export function isGalvoUsbPort(port: string | null | undefined): boolean {
  return !!port && /^usb:galvo:\d+$/.test(port);
}

/** ¿Es un puerto galvo (tarjeta USB o máquina virtual)? */
export function isGalvoPort(port: string | null | undefined): boolean {
  return isGalvoUsbPort(port) || isGalvoVirtualPort(port);
}

/** Número de tarjeta (desde 1) de un puerto `usb:galvo:<n>`; el motor numera desde 0. */
export function galvoBoardNumber(port: string): number {
  return Number(port.slice(GALVO_USB_PREFIX.length)) + 1;
}

/** Los puertos galvo de la lista: primero las tarjetas USB y al final la virtual. */
export function galvoPorts(ports: readonly string[]): string[] {
  return ports.filter(isGalvoPort);
}

/** Solo las tarjetas USB reales encontradas (sin la virtual). */
export function galvoUsbBoards(ports: readonly string[]): string[] {
  return ports.filter(isGalvoUsbPort);
}

/** La lista de puertos sin los del galvo. */
export function withoutGalvoPorts(ports: readonly string[]): string[] {
  return ports.filter((p) => !isGalvoPort(p));
}

/** Longitud de onda típica de cada fuente (nm), para decirle al usuario qué gafas necesita. */
export const SOURCE_WAVELENGTH_NM: Record<LaserSource, number> = {
  fiber_q: 1064,
  fiber_mopa: 1064,
  co2: 10600,
  uv: 355,
};

/** Las tres confirmaciones del diálogo de seguridad: solo si están TODAS se crea el reconocimiento. */
export function completeAck(c: { eyewear: boolean; enclosure: boolean; emergencyStop: boolean }): GalvoSafetyAck | null {
  return c.eyewear && c.enclosure && c.emergencyStop ? { eyewear: true, enclosure: true, emergency_stop: true } : null;
}

/** ¿El mensaje dice que detener no se confirmó? (el motor deja `GALVO_UNCONFIRMED`). */
export function isUnconfirmedStop(...messages: (string | null | undefined)[]): boolean {
  return messages.some((m) => !!m && m.includes(GALVO_UNCONFIRMED));
}

/** Todas las entradas de la tarjeta como bits (de 0 a 15) con su valor, para mostrarlas. */
export function inputBits(inputs: number): { bit: number; on: boolean }[] {
  return Array.from({ length: 16 }, (_, bit) => ({ bit, on: (inputs & (1 << bit)) !== 0 }));
}

/** `0x00A5`. */
export function hex16(n: number): string {
  return `0x${(n & 0xffff).toString(16).toUpperCase().padStart(4, '0')}`;
}
