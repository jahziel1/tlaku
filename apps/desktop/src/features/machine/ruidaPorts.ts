import type { DeviceProfile } from '@/api/types';
import { selectActiveDevice } from '@/store/selectors';
import type { AppState } from '@/store/types';
import { galvoPorts, isGalvoPort } from './galvoPorts';
import { isLhyPort, lhyPorts } from './lhyPorts';
import { baseVirtualPort } from './virtualPort';

/**
 * Ruida (CO2, por red; EXPERIMENTAL). La conexión no usa puerto serie: el motor toma la IP del
 * perfil (`connection.host`). El «puerto» que elige el selector solo distingue la máquina real
 * (`udp://<IP>`, que se muestra) de la virtual (`virtual:ruida`, el emulador UDP del motor).
 */
export const RUIDA_VIRTUAL = 'virtual:ruida';
export const UDP_PREFIX = 'udp://';

/** ¿El perfil activo es una controladora Ruida? */
export function selectIsRuida(s: Pick<AppState, 'devices' | 'activeDeviceId'>): boolean {
  return selectActiveDevice(s).kind === 'ruida';
}

/** IP (o nombre) escrita en el perfil; vacía si el perfil no es de red. */
export function ruidaHost(device: Pick<DeviceProfile, 'connection'>): string {
  return device.connection.type === 'udp' ? device.connection.host.trim() : '';
}

export function isRuidaVirtualPort(port: string | null | undefined): boolean {
  return !!port && baseVirtualPort(port) === RUIDA_VIRTUAL;
}

export function isUdpPort(port: string | null | undefined): boolean {
  return !!port && port.startsWith(UDP_PREFIX);
}

/** Dirección que se muestra de un puerto `udp://IP` (sin el esquema). */
export function udpHost(port: string): string {
  return port.slice(UDP_PREFIX.length);
}

/**
 * La lista de puertos sin las máquinas que no son de serie: la virtual de Ruida y los puertos del
 * K40 (placas USB y su virtual). Es lo que ven GRBL y derivados y el asistente de detección.
 */
export function withoutRuidaPort(ports: readonly string[]): string[] {
  return ports.filter((p) => !isRuidaVirtualPort(p) && !isLhyPort(p) && !isGalvoPort(p));
}

/**
 * Puertos que ofrece el selector para este perfil. Ruida: la IP del perfil (si la hay) y la
 * máquina virtual. Los demás tipos: los de siempre, sin la máquina virtual de Ruida.
 */
export function portsForDevice(device: Pick<DeviceProfile, 'kind' | 'connection'>, ports: readonly string[]): string[] {
  // K40: las placas USB que encontró el motor y la placa simulada.
  if (device.kind === 'lhy_micro') return lhyPorts(ports);
  // Galvo: las tarjetas USB que encontró el motor y la tarjeta simulada.
  if (device.kind === 'galvo') return galvoPorts(ports);
  if (device.kind !== 'ruida') return withoutRuidaPort(ports);
  const host = ruidaHost(device);
  return [...(host ? [`${UDP_PREFIX}${host}`] : []), RUIDA_VIRTUAL];
}

/**
 * Puerto con el que se conecta: el elegido si sirve para este perfil. Con Ruida, lo que quedó de
 * otro tipo de máquina se cambia por la primera opción; con los demás tipos, un resto de Ruida
 * (`udp://` o la virtual) se descarta y se conserva lo demás tal cual.
 */
export function portForDevice(device: Pick<DeviceProfile, 'kind' | 'connection'>, selected: string | null, ports: readonly string[]): string | null {
  const shown = portsForDevice(device, ports);
  if (device.kind === 'lhy_micro' || device.kind === 'galvo') {
    // Un resto de otro tipo de máquina no sirve; una placa USB que ya no está tampoco.
    return selected && shown.includes(selected) ? selected : (shown[0] ?? null);
  }
  if (device.kind === 'ruida') {
    if (selected && shown.some((p) => baseVirtualPort(p) === baseVirtualPort(selected))) return selected;
    return shown[0] ?? null;
  }
  return isUdpPort(selected) || isRuidaVirtualPort(selected) || isLhyPort(selected) || isGalvoPort(selected) ? (shown[0] ?? null) : selected;
}

/** `0x88` o `88` -> 0x88; `null` si no es un byte en hexadecimal. */
export function parseMagic(text: string): number | null {
  const t = text.trim().replace(/^0x/i, '');
  if (!/^[0-9a-f]{1,2}$/i.test(t)) return null;
  return parseInt(t, 16);
}

export function formatMagic(n: number): string {
  return `0x${n.toString(16).toUpperCase().padStart(2, '0')}`;
}

/** Segundos a «12 h 05 min» (o «45 min», «30 s»). */
export function formatDuration(seconds: number): string {
  const s = Math.max(0, Math.round(seconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (h > 0) return `${h} h ${String(m).padStart(2, '0')} min`;
  if (m > 0) return `${m} min`;
  return `${s} s`;
}

/** Puertos que el sistema operativo crea y nunca son una máquina (Bluetooth, consolas de macOS). */
export function isSystemPort(port: string): boolean {
  return /bluetooth|debug-console|wlan-debug|airpods|-wireless|rfcomm/i.test(port);
}

/**
 * El puerto que se elige solo al listar: el primero que parece una máquina real (USB-serie, `COMn`).
 * Nunca un puerto del sistema (en macOS el primero de la lista era `/dev/cu.debug-console` y la app
 * intentaba conectarse ahí), ni una máquina virtual o de red. `null` si no hay ninguno: que el
 * usuario elija.
 */
export function defaultPort(ports: readonly string[]): string | null {
  return ports.find((p) => !isSystemPort(p) && !/^(virtual|virtualnet|udp|usb):/i.test(p)) ?? null;
}
