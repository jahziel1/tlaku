import { polylineBez } from '@/lib/bezpath';
import { DEFAULT_ROTARY, kindDefaults } from '@/lib/device';
import { makeLayer } from '@/lib/layers';
import type { LaserApi } from '../api';
import type { DeviceProfile, Document } from '../types';

type Lag = <T>(value: () => T) => Promise<T>;

/**
 * Perfil por defecto del motor (`DeviceProfile::default()` de tlaku-doc): un láser de diodo con
 * GRBL de fábrica y cama de 400 x 415 mm. Es la base de `deviceDefaults`, como en Rust.
 */
export function engineDefaultDevice(): DeviceProfile {
  const grbl = kindDefaults('grbl');
  return {
    id: 'diodo-grbl-400x415',
    name: 'Diodo GRBL 400x415',
    ...grbl,
    bed: [400, 415],
    origin: 'bottom_left',
    accel: [1000, 1000],
    max_rate: [12000, 12000],
    rapid_rate: 12000,
    rx_buffer: 127,
    start_gcode: '',
    end_gcode: '',
    return_to_origin: true,
    z_axis: false,
    z_invert: false,
    rotary: { ...DEFAULT_ROTARY },
    scan_offsets: [],
    scan_initial_offset: 0,
  };
}

/** `DeviceProfile::for_kind`: el perfil por defecto con el dialecto, `s_max`, modo de potencia y baudios del tipo. */
export function mockDeviceDefaults(kind: DeviceProfile['kind']): DeviceProfile {
  return { ...engineDefaultDevice(), ...kindDefaults(kind) };
}

const positive = (v: number) => Number.isFinite(v) && v > 0;

/**
 * Prueba de una vuelta equivalente a `tlaku_pipeline::rotary_test`: una línea en Y de exactamente
 * una circunferencia de la pieza (`pi * object_diameter`), a poca potencia, con los mismos errores.
 */
export function mockRotaryTest(device: DeviceProfile, speed: number, power: number): Document {
  const r = device.rotary;
  if (!r.enabled) throw new Error(`activa el rotativo de la máquina '${device.name}' para generar la prueba de una vuelta`);
  if (!positive(r.mm_per_rotation)) throw new Error(`la máquina '${device.name}': rotativo: mm por vuelta debe ser mayor que 0`);
  if (r.kind === 'roller' && !positive(r.roller_diameter)) throw new Error(`la máquina '${device.name}': rotativo: el diámetro del rodillo debe ser mayor que 0`);
  if (r.kind === 'chuck' && !positive(r.object_diameter)) throw new Error(`la máquina '${device.name}': rotativo: el diámetro de la pieza debe ser mayor que 0`);
  if (!positive(r.object_diameter)) {
    throw new Error(`la máquina '${device.name}': el diámetro de la pieza debe ser mayor que 0 para calcular la circunferencia de la prueba`);
  }
  if (!positive(speed)) throw new Error(`la velocidad de la prueba debe ser mayor que 0: ${speed}`);
  if (!(positive(power) && power <= 100)) throw new Error(`la potencia de la prueba debe estar entre 0 y 100 %: ${power}`);
  const color = '#ff0000';
  const x = Math.min(10, device.bed[0] / 2);
  return {
    version: 2,
    objects: [
      {
        id: 'prueba-una-vuelta',
        layer: color,
        transform: [1, 0, 0, 1, 0, 0],
        kind: { type: 'vector', paths: [polylineBez([[x, 0], [x, Math.PI * r.object_diameter]], false)] },
        locked: false,
        hidden: false,
        group: null,
      },
    ],
    layers: [makeLayer(color, 'Prueba de una vuelta', { speed, power })],
    device_id: null,
  };
}

export function mockDevices(lag: Lag): Pick<LaserApi, 'deviceDefaults' | 'rotaryTest'> {
  return {
    deviceDefaults: ({ kind }) => lag(() => mockDeviceDefaults(kind)),
    rotaryTest: ({ device, speed, power }) => lag(() => mockRotaryTest(device, speed, power)),
  };
}
