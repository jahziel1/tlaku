import type { Connection } from '@/bindings/Connection';

/**
 * Conexión de la máquina por red (Telnet/TCP y WebSocket), parte pura: validación, el texto que se
 * entrega al motor en lugar del puerto serie y la dirección que se muestra. Los componentes
 * (`DeviceConnection.tsx`) solo pintan.
 *
 * El motor reconoce estas direcciones por su esquema (`tcp://host:puerto`, `ws://host:puerto/ruta`);
 * un puerto serie nunca empieza así. Ver `src-tauri/src/netlink.rs`.
 */

/** Tipos que se editan aquí. `udp` (Ruida) tiene su propio campo y no se toca. */
export type NetConnectionType = 'serial' | 'tcp' | 'web_socket';
export const CONNECTION_TYPES: readonly NetConnectionType[] = ['serial', 'tcp', 'web_socket'];

/** Puerto Telnet, el de grblHAL Ethernet y FluidNC por omisión. */
export const DEFAULT_TCP_PORT = 23;

export type ConnectionIssue = 'host' | 'port' | 'url' | 'wss';

const HOSTNAME = /^[A-Za-z0-9]([A-Za-z0-9_-]*[A-Za-z0-9])?(\.[A-Za-z0-9]([A-Za-z0-9_-]*[A-Za-z0-9])?)*$/;
const IPV6 = /^[0-9A-Fa-f:.]+$/;

/** ¿Es un nombre de equipo, una IPv4 o una IPv6 (sin corchetes) razonable? */
export function isValidHost(host: string): boolean {
  const h = host.trim();
  if (h === '' || h.length > 253) return false;
  if (h.includes(':')) return IPV6.test(h) && h.split(':').length >= 3;
  return HOSTNAME.test(h);
}

function isValidPort(port: number): boolean {
  return Number.isInteger(port) && port >= 1 && port <= 65535;
}

/** Problema de la dirección ws:// o wss://, o null si está bien. */
function urlIssue(url: string): ConnectionIssue | null {
  const u = url.trim();
  if (/^wss:\/\//i.test(u)) return 'wss';
  const m = /^ws:\/\/(\[[0-9A-Fa-f:.]+\]|[^/:?#\s]+)(?::(\d*))?([/?#].*)?$/i.exec(u);
  if (!m) return 'url';
  const host = m[1]!;
  if (host.startsWith('[') ? !isValidHost(host.slice(1, -1)) : !isValidHost(host)) return 'url';
  if (m[2] !== undefined && !(m[2] !== '' && isValidPort(Number(m[2])))) return 'url';
  return null;
}

/** Errores de la conexión del perfil (`null` si es válida). Serie y UDP no tienen nada que validar aquí. */
export function connectionIssue(c: Connection | undefined): ConnectionIssue | null {
  if (!c) return null;
  switch (c.type) {
    case 'tcp':
      if (!isValidHost(c.host)) return 'host';
      return isValidPort(c.port) ? null : 'port';
    case 'web_socket':
      return urlIssue(c.url);
    case 'serial':
    case 'udp':
      return null;
  }
}

/** La conexión se hace por red (TCP o WebSocket): ahí no se elige puerto serie. */
export function isNetworkConnection(c: Connection | undefined): c is Extract<Connection, { type: 'tcp' | 'web_socket' }> {
  return c?.type === 'tcp' || c?.type === 'web_socket';
}

/** Dirección corta para mostrar: `192.168.1.50:23` o la URL del WebSocket. */
export function connectionAddress(c: Connection | undefined): string | null {
  if (c?.type === 'tcp') return `${c.host.includes(':') ? `[${c.host}]` : c.host}:${c.port}`;
  if (c?.type === 'web_socket') return c.url.trim();
  return null;
}

/**
 * Lo que se entrega al motor en lugar del puerto serie: `tcp://host:puerto` o la URL `ws://`.
 * `null` para serie (se usa el puerto elegido) y para UDP.
 */
export function connectionPortId(c: Connection | undefined): string | null {
  const address = connectionAddress(c);
  if (!address) return null;
  return c?.type === 'tcp' ? `tcp://${address}` : address;
}

/** ¿`port` (el texto que viaja como puerto) es una dirección de red? */
export function isNetworkPortId(port: string | null | undefined): boolean {
  return !!port && /^(tcp|wss?):\/\//i.test(port);
}

/** Cambia el tipo de conexión conservando lo que se pueda (el nombre de host pasa a la URL y al revés). */
export function withConnectionType(c: Connection | undefined, type: NetConnectionType): Connection {
  if (c?.type === type) return c;
  switch (type) {
    case 'serial':
      return { type: 'serial' };
    case 'tcp': {
      const host = c?.type === 'web_socket' ? (/^ws:\/\/\[?([^\]/:?#]+)/i.exec(c.url)?.[1] ?? '') : '';
      return { type: 'tcp', host, port: DEFAULT_TCP_PORT };
    }
    case 'web_socket': {
      const host = c?.type === 'tcp' ? c.host : '';
      return { type: 'web_socket', url: host ? `ws://${host.includes(':') ? `[${host}]` : host}:81` : 'ws://' };
    }
  }
}
