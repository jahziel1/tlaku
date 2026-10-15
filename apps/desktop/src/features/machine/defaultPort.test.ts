import { describe, expect, it } from 'vitest';
import { defaultPort, isSystemPort } from './ruidaPorts';

describe('puerto por omisión', () => {
  it('en macOS no elige la consola de depuración: elige la máquina USB', () => {
    // El orden real que dio list_ports en una Mac con una Falcon2 conectada.
    const ports = ['/dev/cu.debug-console', '/dev/cu.wlan-debug', '/dev/cu.Bluetooth-Incoming-Port', '/dev/cu.usbmodem1234561', 'virtual:falcon2'];
    expect(defaultPort(ports)).toBe('/dev/cu.usbmodem1234561');
  });

  it('sin máquina real no elige nada (ni el sistema ni una virtual)', () => {
    expect(defaultPort(['/dev/cu.debug-console', '/dev/cu.Bluetooth-Incoming-Port', 'virtual:falcon2', 'virtualnet:grbl11', 'usb:k40:0'])).toBeNull();
  });

  it('Windows y Linux', () => {
    expect(defaultPort(['COM3', 'virtual:falcon2'])).toBe('COM3');
    expect(defaultPort(['/dev/ttyUSB0'])).toBe('/dev/ttyUSB0');
    expect(isSystemPort('/dev/rfcomm0')).toBe(true);
    expect(isSystemPort('/dev/ttyACM0')).toBe(false);
  });
});
