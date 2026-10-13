import { describe, expect, it } from 'vitest';
import { baseVirtualPort, isVirtualNetPort, isVirtualPort, portKey, portLabel, virtualSpeed, withVirtualSpeed } from './virtualPort';

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

describe('máquinas virtuales por red', () => {
  it('se reconocen como virtuales, con su prefijo propio', () => {
    expect(isVirtualPort('virtualnet:falcon2')).toBe(true);
    expect(isVirtualNetPort('virtualnet:falcon2@5')).toBe(true);
    expect(isVirtualNetPort('virtual:falcon2')).toBe(false);
    expect(isVirtualNetPort('tcp://10.0.0.5:23')).toBe(false);
  });

  it('la velocidad y la identidad funcionan igual que en las locales', () => {
    expect(withVirtualSpeed('virtualnet:falcon2', 5)).toBe('virtualnet:falcon2@5');
    expect(virtualSpeed('virtualnet:fluidnc@20')).toBe(20);
    expect(portKey('virtualnet:falcon2@5')).toBe(portKey('virtualnet:falcon2'));
    expect(portKey('virtualnet:falcon2')).not.toBe(portKey('virtual:falcon2'));
  });

  it('se muestran con nombre claro; las direcciones de red, sin el esquema tcp://', () => {
    expect(portLabel('virtualnet:falcon2')).toBe('Máquina virtual por red · Falcon2 Pro');
    expect(portLabel('virtualnet:fluidnc@5')).toBe('Máquina virtual por red · FluidNC (×5)');
    expect(portLabel('tcp://192.168.1.50:23')).toBe('192.168.1.50:23');
    expect(portLabel('ws://fluidnc.local:81/')).toBe('ws://fluidnc.local:81/');
  });
});
