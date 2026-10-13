import { describe, expect, it } from 'vitest';
import { baseVirtualPort, isVirtualPort, portKey, portLabel, virtualSpeed, withVirtualSpeed } from './virtualPort';

describe('puertos virtuales', () => {
  it('se reconocen por el prefijo, que ningún puerto real usa', () => {
    expect(isVirtualPort('virtual:falcon2')).toBe(true);
    expect(isVirtualPort('virtual:fluidnc@5')).toBe(true);
    for (const real of ['/dev/cu.usbserial-1420', '/dev/ttyUSB0', 'COM3', '', null, undefined]) expect(isVirtualPort(real)).toBe(false);
  });

  it('la velocidad viaja en el id: ×1 no se escribe', () => {
    expect(withVirtualSpeed('virtual:falcon2', 5)).toBe('virtual:falcon2@5');
    expect(withVirtualSpeed('virtual:falcon2', 1)).toBe('virtual:falcon2');
    // Cambiar la velocidad no acumula sufijos.
    expect(withVirtualSpeed('virtual:falcon2@5', 20)).toBe('virtual:falcon2@20');
    expect(withVirtualSpeed('virtual:falcon2@5', 1)).toBe('virtual:falcon2');
    expect(virtualSpeed('virtual:falcon2@20')).toBe(20);
    expect(virtualSpeed('virtual:falcon2')).toBe(1);
    expect(virtualSpeed('virtual:falcon2@x')).toBe(1);
    expect(baseVirtualPort('virtual:grblhal@5')).toBe('virtual:grblhal');
  });

  it('la misma máquina es la misma sin importar la velocidad', () => {
    expect(portKey('virtual:falcon2@5')).toBe(portKey('virtual:falcon2'));
    expect(portKey('virtual:falcon2')).not.toBe(portKey('virtual:grbl11'));
    expect(portKey('/dev/ttyUSB0')).toBe('/dev/ttyUSB0');
    expect(portKey(null)).toBeNull();
  });

  it('las virtuales se muestran con un nombre claro; las reales, tal cual', () => {
    expect(portLabel('virtual:falcon2')).toBe('Máquina virtual · Falcon2 Pro');
    expect(portLabel('virtual:grbl11')).toBe('Máquina virtual · GRBL 1.1');
    expect(portLabel('virtual:grblhal')).toBe('Máquina virtual · grblHAL');
    expect(portLabel('virtual:fluidnc')).toBe('Máquina virtual · FluidNC');
    expect(portLabel('virtual:marlin')).toBe('Máquina virtual · Marlin');
    expect(portLabel('virtual:smoothie')).toBe('Máquina virtual · Smoothieware');
    expect(portLabel('virtual:falcon2@5')).toBe('Máquina virtual · Falcon2 Pro (×5)');
    expect(portLabel('virtual:otra')).toBe('Máquina virtual · otra');
    expect(portLabel('/dev/cu.usbserial-1420')).toBe('/dev/cu.usbserial-1420');
  });
});
