import { describe, expect, it } from 'vitest';
import { createMockApi } from './index';
import { mockDeviceDefaults, mockRotaryTest } from './devices';
import { sampleDevices } from './sample';

const api = createMockApi({ latencyMs: 0 });

describe('device_defaults simulado', () => {
  // Los mismos valores que prueba `defaults_follow_the_kind_table` en src-tauri/src/edit/devices.rs.
  it('Marlin: S 255, 250000 baudios, potencia constante y M3 S0 I', async () => {
    const d = await api.deviceDefaults({ kind: 'marlin' });
    expect(d.kind).toBe('marlin');
    expect([d.s_max, d.baud, d.power_mode]).toEqual([255, 250_000, 'constant']);
    expect(d.dialect.constant_on).toBe('M3 S0 I');
    expect(d.dialect.dynamic_on).toBeNull();
  });

  it('Smoothie: S 1, 115200 baudios y cierre con M400', async () => {
    const d = await api.deviceDefaults({ kind: 'smoothie' });
    expect([d.s_max, d.baud, d.power_mode]).toEqual([1, 115_200, 'constant']);
    expect(d.dialect.end_sync).toBe('M400');
  });

  it('GRBL es el perfil por defecto del motor y el resto de campos no depende del tipo', async () => {
    const grbl = await api.deviceDefaults({ kind: 'grbl' });
    expect(grbl).toMatchObject({ id: 'diodo-grbl-400x415', bed: [400, 415], s_max: 1000, power_mode: 'dynamic' });
    const hal = mockDeviceDefaults('grbl_hal');
    expect({ ...hal, kind: 'grbl' }).toEqual(grbl);
  });
});

describe('rotary_test simulado', () => {
  const rotary = (patch = {}) => {
    const d = sampleDevices()[0]!;
    return { ...d, rotary: { ...d.rotary, enabled: true, object_diameter: 80, ...patch } };
  };

  it('una línea en Y de exactamente una circunferencia, a poca potencia', async () => {
    const doc = await api.rotaryTest({ device: rotary(), speed: 1500, power: 10 });
    expect(doc.layers).toHaveLength(1);
    expect(doc.layers[0]).toMatchObject({ speed: 1500, power: 10, name: 'Prueba de una vuelta' });
    const path = (doc.objects[0]!.kind as { paths: { type: string; to?: [number, number] }[][] }).paths[0]!;
    expect(path[0]!.to).toEqual([10, 0]);
    expect(path[1]!.to![0]).toBe(10);
    expect(path[1]!.to![1]).toBeCloseTo(Math.PI * 80, 9);
  });

  it('falla con el rotativo apagado o con valores no válidos, con los mensajes del motor', () => {
    const off = rotary({ enabled: false });
    expect(() => mockRotaryTest(off, 1000, 5)).toThrow('activa el rotativo');
    expect(() => mockRotaryTest(rotary({ mm_per_rotation: 0 }), 1000, 5)).toThrow('mm por vuelta debe ser mayor que 0');
    // Con mandril el diámetro de la pieza ya es obligatorio; con rodillos solo lo pide la prueba.
    expect(() => mockRotaryTest(rotary({ object_diameter: 0 }), 1000, 5)).toThrow('el diámetro de la pieza debe ser mayor que 0');
    expect(() => mockRotaryTest(rotary({ kind: 'roller', object_diameter: 0 }), 1000, 5)).toThrow('circunferencia');
    expect(() => mockRotaryTest(rotary(), 0, 5)).toThrow('velocidad');
    expect(() => mockRotaryTest(rotary(), 1000, 101)).toThrow('potencia');
  });
});

describe('perfiles de ejemplo', () => {
  it('la Falcon2 sigue el perfil recomendado de docs/hardware/falcon2.md', () => {
    const f = sampleDevices().find((d) => d.id === 'falcon2-22w')!;
    expect(f).toMatchObject({ bed: [400, 415], s_max: 1000, power_mode: 'dynamic', max_rate: [25000, 25000], rapid_rate: 25000, baud: 115200, rx_buffer: 127, return_to_origin: true, z_axis: false });
    expect(f.rotary).toMatchObject({ kind: 'chuck', mm_per_rotation: 40, enabled: false });
  });

  it('hay un perfil que no es GRBL para que se vea el selector', () => {
    expect(new Set(sampleDevices().map((d) => d.kind)).size).toBeGreaterThan(1);
  });
});
