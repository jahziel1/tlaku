import { describe, expect, it } from 'vitest';
import type { DeviceProfile } from '@/api/types';
import { FALLBACK_DEVICE } from '@/lib/device';
import {
  isLhyPort,
  isLhyUsbPort,
  isLhyVirtualPort,
  lhyBoardNumber,
  lhyPorts,
  lhyUsbBoards,
  selectIsLhy,
  statusKey,
  withoutLhyPorts,
} from './lhyPorts';
import { portForDevice, portsForDevice, withoutRuidaPort } from './ruidaPorts';
import { portLabel } from './virtualPort';

const k40: DeviceProfile = { ...FALLBACK_DEVICE, id: 'k40', kind: 'lhy_micro', bed: [300, 200], origin: 'top_left' };
const PORTS = ['/dev/ttyUSB0', 'usb:k40:0', 'usb:k40:1', 'virtual:falcon2', 'virtual:ruida', 'virtual:k40'];

describe('puertos del K40', () => {
  it('reconoce las placas USB y la máquina virtual (con o sin velocidad)', () => {
    expect(isLhyUsbPort('usb:k40:0')).toBe(true);
    expect(isLhyUsbPort('usb:k40:')).toBe(false);
    expect(isLhyUsbPort('usb:k40:x')).toBe(false);
    expect(isLhyVirtualPort('virtual:k40')).toBe(true);
    expect(isLhyVirtualPort('virtual:k40@5')).toBe(true);
    expect(isLhyVirtualPort('virtual:ruida')).toBe(false);
    expect(isLhyPort('/dev/ttyUSB0')).toBe(false);
    expect(isLhyPort(null)).toBe(false);
  });

  it('un perfil K40 solo ve sus placas y su máquina virtual', () => {
    expect(portsForDevice(k40, PORTS)).toEqual(['usb:k40:0', 'usb:k40:1', 'virtual:k40']);
    expect(lhyPorts(PORTS)).toEqual(['usb:k40:0', 'usb:k40:1', 'virtual:k40']);
    expect(lhyUsbBoards(PORTS)).toEqual(['usb:k40:0', 'usb:k40:1']);
  });

  it('los demás tipos no ven nada del K40', () => {
    expect(portsForDevice(FALLBACK_DEVICE, PORTS)).toEqual(['/dev/ttyUSB0', 'virtual:falcon2']);
    expect(withoutRuidaPort(PORTS)).toEqual(['/dev/ttyUSB0', 'virtual:falcon2']);
    expect(withoutLhyPorts(PORTS)).toEqual(['/dev/ttyUSB0', 'virtual:falcon2', 'virtual:ruida']);
    // Ruida tampoco ofrece los puertos del K40.
    expect(portsForDevice({ ...FALLBACK_DEVICE, kind: 'ruida', connection: { type: 'udp', host: '' } }, PORTS)).toEqual(['virtual:ruida']);
  });

  it('el puerto con que se conecta sale del perfil, no de un resto de otro tipo de máquina', () => {
    expect(portForDevice(k40, '/dev/ttyUSB0', PORTS)).toBe('usb:k40:0');
    expect(portForDevice(k40, 'virtual:k40', PORTS)).toBe('virtual:k40');
    expect(portForDevice(k40, 'usb:k40:7', PORTS)).toBe('usb:k40:0');
    expect(portForDevice(k40, null, ['/dev/ttyUSB0'])).toBeNull();
    // Un perfil GRBL conserva su puerto y descarta lo que quedó del K40.
    expect(portForDevice(FALLBACK_DEVICE, '/dev/ttyUSB0', PORTS)).toBe('/dev/ttyUSB0');
    expect(portForDevice(FALLBACK_DEVICE, 'usb:k40:0', PORTS)).toBe('/dev/ttyUSB0');
    expect(portForDevice(FALLBACK_DEVICE, 'virtual:k40', PORTS)).toBe('/dev/ttyUSB0');
  });

  it('los nombres para mostrar', () => {
    expect(lhyBoardNumber('usb:k40:0')).toBe(1);
    expect(portLabel('usb:k40:1')).toBe('K40 · placa USB 2');
    expect(portLabel('virtual:k40')).toBe('Máquina virtual · K40 (M2 Nano)');
  });

  it('el perfil activo decide si el panel es del K40', () => {
    const s = { devices: [FALLBACK_DEVICE, k40], activeDeviceId: 'k40' };
    expect(selectIsLhy(s)).toBe(true);
    expect(selectIsLhy({ ...s, activeDeviceId: FALLBACK_DEVICE.id })).toBe(false);
  });

  it('los códigos de estado de la placa tienen texto; los desconocidos no', () => {
    for (const c of [206, 238, 236, 207, 239, 204, 0]) expect(statusKey(c), String(c)).not.toBeNull();
    expect(statusKey(77)).toBeNull();
  });
});
