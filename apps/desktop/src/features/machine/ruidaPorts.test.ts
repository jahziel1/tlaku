import { describe, expect, it } from 'vitest';
import { FALLBACK_DEVICE } from '@/lib/device';
import type { DeviceProfile } from '@/api/types';
import { formatDuration, formatMagic, isRuidaVirtualPort, parseMagic, portForDevice, portsForDevice, ruidaHost } from './ruidaPorts';

const ruida = (host: string): DeviceProfile => ({ ...FALLBACK_DEVICE, kind: 'ruida', connection: { type: 'udp', host } });
const SERIAL = ['/dev/ttyUSB0', 'virtual:falcon2', 'virtual:ruida'];

describe('puertos de Ruida', () => {
  it('la IP del perfil y la máquina virtual son las únicas opciones de un perfil Ruida', () => {
    expect(portsForDevice(ruida('192.168.1.100'), SERIAL)).toEqual(['udp://192.168.1.100', 'virtual:ruida']);
    // Sin IP solo queda la virtual.
    expect(portsForDevice(ruida('  '), SERIAL)).toEqual(['virtual:ruida']);
    expect(ruidaHost(ruida(' 10.0.0.5 '))).toBe('10.0.0.5');
    expect(ruidaHost(FALLBACK_DEVICE)).toBe('');
  });

  it('los demás tipos no ven la máquina virtual de Ruida', () => {
    expect(portsForDevice(FALLBACK_DEVICE, SERIAL)).toEqual(['/dev/ttyUSB0', 'virtual:falcon2']);
    expect(isRuidaVirtualPort('virtual:ruida@5')).toBe(true);
    expect(isRuidaVirtualPort('virtual:falcon2')).toBe(false);
  });

  it('el puerto con que se conecta sale del perfil, no de un resto de otro tipo de máquina', () => {
    const d = ruida('192.168.1.100');
    expect(portForDevice(d, '/dev/ttyUSB0', SERIAL)).toBe('udp://192.168.1.100');
    expect(portForDevice(d, 'virtual:ruida@5', SERIAL)).toBe('virtual:ruida@5');
    expect(portForDevice(d, null, SERIAL)).toBe('udp://192.168.1.100');
    // Un perfil GRBL conserva su puerto y descarta lo que quedó de Ruida.
    expect(portForDevice(FALLBACK_DEVICE, '/dev/ttyUSB0', SERIAL)).toBe('/dev/ttyUSB0');
    expect(portForDevice(FALLBACK_DEVICE, 'udp://1.2.3.4', SERIAL)).toBe('/dev/ttyUSB0');
    expect(portForDevice(FALLBACK_DEVICE, 'virtual:ruida', SERIAL)).toBe('/dev/ttyUSB0');
    expect(portForDevice(FALLBACK_DEVICE, null, SERIAL)).toBeNull();
  });

  it('el magic se lee en hexadecimal con o sin 0x', () => {
    expect(parseMagic('0x88')).toBe(0x88);
    expect(parseMagic(' 11 ')).toBe(0x11);
    expect(parseMagic('0X3a')).toBe(0x3a);
    for (const bad of ['', 'zz', '0x', '123', '0x123', '-1']) expect(parseMagic(bad), bad).toBeNull();
    expect(formatMagic(0x8)).toBe('0x08');
    expect(formatMagic(0x88)).toBe('0x88');
  });

  it('las duraciones se escriben cortas', () => {
    expect(formatDuration(30)).toBe('30 s');
    expect(formatDuration(3 * 60 + 10)).toBe('3 min');
    expect(formatDuration(7200 + 5 * 60)).toBe('2 h 05 min');
    expect(formatDuration(-4)).toBe('0 s');
  });
});
