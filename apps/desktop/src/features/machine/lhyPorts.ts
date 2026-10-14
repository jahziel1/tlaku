import type { DeviceProfile } from '@/api/types';
import { selectActiveDevice } from '@/store/selectors';
import type { AppState } from '@/store/types';
import { baseVirtualPort } from './virtualPort';

/**
 * K40 con placa M2 Nano (EXPERIMENTAL). No usa puerto serie: el motor habla con la placa por USB
 * (CH341, 1A86:5512). La lista de puertos trae una entrada por placa encontrada (`usb:k40:<n>`) y
 * la máquina virtual (`virtual:k40`, la placa simulada del motor). Solo se ofrecen con un perfil K40.
 */
export const LHY_VIRTUAL = 'virtual:k40';
export const LHY_USB_PREFIX = 'usb:k40:';

/** ¿El perfil activo es un K40 con M2 Nano? */
export function selectIsLhy(s: Pick<AppState, 'devices' | 'activeDeviceId'>): boolean {
  return selectActiveDevice(s).kind === 'lhy_micro';
}

export function isLhyVirtualPort(port: string | null | undefined): boolean {
  return !!port && baseVirtualPort(port) === LHY_VIRTUAL;
}

export function isLhyUsbPort(port: string | null | undefined): boolean {
  return !!port && /^usb:k40:\d+$/.test(port);
}

/** ¿Es un puerto del K40 (placa USB o máquina virtual)? */
export function isLhyPort(port: string | null | undefined): boolean {
  return isLhyUsbPort(port) || isLhyVirtualPort(port);
}

/** Número de placa (desde 1) de un puerto `usb:k40:<n>`; el motor numera desde 0. */
export function lhyBoardNumber(port: string): number {
  return Number(port.slice(LHY_USB_PREFIX.length)) + 1;
}

/** Los puertos del K40 de la lista: primero las placas USB y al final la virtual. */
export function lhyPorts(ports: readonly string[]): string[] {
  return ports.filter(isLhyPort);
}

/** Solo las placas USB reales encontradas (sin la virtual). */
export function lhyUsbBoards(ports: readonly string[]): string[] {
  return ports.filter(isLhyUsbPort);
}

/** La lista de puertos sin los del K40 (los de serie, el asistente de detección, etc.). */
export function withoutLhyPorts(ports: readonly string[]): string[] {
  return ports.filter((p) => !isLhyPort(p));
}

/** El perfil activo es un K40: lo que sigue falla con los demás tipos. */
export function isLhyDevice(device: Pick<DeviceProfile, 'kind'>): boolean {
  return device.kind === 'lhy_micro';
}

/** Modelos de placa que ofrece el perfil (los que acepta el motor). La M2 es la de serie. */
export const LHY_BOARDS = ['M2', 'M1', 'M', 'M3', 'A', 'B', 'B1', 'B2'] as const;
export const LHY_DEFAULT_BOARD = 'M2';

/** Texto del código de estado de la placa; los que no se conocen salen como «Código N». */
export function statusKey(code: number): 'lhymicro.status.206' | 'lhymicro.status.238' | 'lhymicro.status.236' | 'lhymicro.status.207' | 'lhymicro.status.239' | 'lhymicro.status.204' | 'lhymicro.status.0' | null {
  switch (code) {
    case 206: return 'lhymicro.status.206';
    case 238: return 'lhymicro.status.238';
    case 236: return 'lhymicro.status.236';
    case 207: return 'lhymicro.status.207';
    case 239: return 'lhymicro.status.239';
    case 204: return 'lhymicro.status.204';
    case 0: return 'lhymicro.status.0';
    default: return null;
  }
}
