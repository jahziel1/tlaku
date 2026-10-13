import { describe, expect, it } from 'vitest';
import type { DeviceProfile } from '@/api/types';
import {
  applyProposal,
  docDeltaToWork,
  docToWork,
  duplicateDeviceProfile,
  FALLBACK_DEVICE,
  newDeviceProfile,
  parseGrblSettings,
  parseGrblVersion,
  proposeFromSettings,
  sameDevice,
  slugify,
  uniqueDeviceId,
  validateDevice,
  workToDoc,
} from './device';

const device = (patch: Partial<DeviceProfile> = {}): DeviceProfile => ({ ...FALLBACK_DEVICE, ...patch });

const DOLLAR_DOLLAR = [
  '$0=10',
  '$22=1',
  '$30=1000',
  '$31=0',
  '$32=1',
  '$100=80.000',
  '$110=12000.000',
  '$111=9000.000',
  '$120=500.000',
  '$121=400.000',
  '$130=410.000',
  '$131=400.000',
  'ok',
];

describe('parseGrblSettings', () => {
  it('lee las líneas $N=valor y descarta lo demás', () => {
    const s = parseGrblSettings(['Grbl 1.1h', ...DOLLAR_DOLLAR, '[MSG:algo]', '$x=3']);
    expect(s[130]).toBe(410);
    expect(s[110]).toBe(12000);
    expect(s[30]).toBe(1000);
    expect(s[32]).toBe(1);
    expect(Object.keys(s)).toHaveLength(12);
  });

  it('acepta negativos, espacios y comentarios al final', () => {
    expect(parseGrblSettings(['$23=-1', ' $5 = 0 (invert)', '$$'])).toEqual({ 23: -1, 5: 0 });
  });
});

describe('proposeFromSettings', () => {
  it('propone área, velocidad, aceleración y S máx. desde $130/$131, $110/$111, $120/$121 y $30', () => {
    const p = proposeFromSettings(parseGrblSettings(DOLLAR_DOLLAR), device({ s_max: 255 }));
    const byField = Object.fromEntries(p.changes.map((c) => [c.field, c.to]));
    expect(byField).toEqual({ bed: [410, 400], max_rate: [12000, 9000], accel: [500, 400], s_max: 1000 });
    expect(p.laserMode).toBe(true);
    expect(p.nothingFound).toBe(false);
    expect(p.inSync).toBe(false);
  });

  it('no propone lo que ya coincide y lo marca como sincronizado', () => {
    const current = device({ bed: [410, 400], max_rate: [12000, 9000], accel: [500, 400], s_max: 1000 });
    const p = proposeFromSettings(parseGrblSettings(DOLLAR_DOLLAR), current);
    expect(p.changes).toEqual([]);
    expect(p.inSync).toBe(true);
  });

  it('avisa si el modo láser está desactivado ($32=0)', () => {
    const p = proposeFromSettings(parseGrblSettings(['$32=0', '$30=1000']), device());
    expect(p.laserMode).toBe(false);
  });

  it('con $32 ausente no afirma nada sobre el modo láser', () => {
    expect(proposeFromSettings({ 30: 255 }, device()).laserMode).toBeNull();
  });

  it('si solo viene un eje conserva el valor actual del otro', () => {
    const p = proposeFromSettings({ 130: 300 }, device({ bed: [400, 350] }));
    expect(p.changes).toEqual([{ field: 'bed', codes: '$130/$131', from: [400, 350], to: [300, 350] }]);
  });

  it('ignora valores no positivos y avisa si no hay nada que interpretar', () => {
    const p = proposeFromSettings({ 130: 0, 131: -5, 0: 10 }, device());
    expect(p.changes).toEqual([]);
    expect(p.nothingFound).toBe(true);
    expect(p.inSync).toBe(false);
  });

  it('applyProposal solo toca los campos elegidos', () => {
    const p = proposeFromSettings(parseGrblSettings(DOLLAR_DOLLAR), device({ s_max: 255 }));
    const only = applyProposal(device({ s_max: 255 }), p.changes.filter((c) => c.field === 'bed'));
    expect(only.bed).toEqual([410, 400]);
    expect(only.s_max).toBe(255);
    expect(only.max_rate).toEqual(FALLBACK_DEVICE.max_rate);
  });
});

describe('validateDevice', () => {
  it('acepta el perfil de fábrica', () => {
    expect(validateDevice(FALLBACK_DEVICE)).toEqual({});
  });

  it('marca cada campo inválido', () => {
    const bad = validateDevice(device({ name: '  ', bed: [0, 400], s_max: 0, accel: [100, -1], max_rate: [0, 0], rapid_rate: 0, baud: 0, rx_buffer: 8 }));
    expect(bad).toEqual({
      name: 'required',
      bed: 'positive',
      s_max: 'positive',
      accel: 'positive',
      max_rate: 'positive',
      rapid_rate: 'positive',
      baud: 'positive',
      rx_buffer: 'tooSmall',
    });
  });

  it('S máx. debe ser entera y la cama razonable', () => {
    expect(validateDevice(device({ s_max: 10.5 })).s_max).toBe('integer');
    expect(validateDevice(device({ bed: [20000, 10] })).bed).toBe('tooBig');
    expect(validateDevice(device({ rx_buffer: 127.5 })).rx_buffer).toBe('integer');
  });
});

describe('alta y copia', () => {
  it('slugify quita acentos y símbolos', () => {
    expect(slugify('Láser Diodo #2 (grande)')).toBe('laser-diodo-2-grande');
    expect(slugify('***')).toBe('maquina');
  });

  it('los ids no se repiten', () => {
    expect(uniqueDeviceId('Mi láser', ['mi-laser', 'mi-laser-2'])).toBe('mi-laser-3');
    expect(uniqueDeviceId('Otro', ['mi-laser'])).toBe('otro');
  });

  it('un perfil nuevo trae valores de fábrica y id único', () => {
    const d = newDeviceProfile('Mi láser', ['mi-laser']);
    expect(d.id).toBe('mi-laser-2');
    expect(d.name).toBe('Mi láser');
    expect(d.bed).toEqual(FALLBACK_DEVICE.bed);
  });

  it('duplicar copia todo menos id y nombre, sin compartir arreglos', () => {
    const src = device({ id: 'a', name: 'A', bed: [300, 200], end_gcode: 'M5\nG0 X0 Y0' });
    const copy = duplicateDeviceProfile(src, '(copia)', ['a']);
    expect(copy.name).toBe('A (copia)');
    expect(copy.id).toBe('a-copia');
    expect(copy.bed).toEqual([300, 200]);
    expect(copy.end_gcode).toBe(src.end_gcode);
    copy.bed[0] = 1;
    expect(src.bed[0]).toBe(300);
    expect(sameDevice(src, src)).toBe(true);
    expect(sameDevice(src, copy)).toBe(false);
  });
});

describe('coordenadas según el origen del perfil', () => {
  const bed = [400, 300] as [number, number];
  it('abajo a la izquierda no cambia nada', () => {
    expect(workToDoc({ bed, origin: 'bottom_left' }, [10, 20])).toEqual([10, 20]);
    expect(docDeltaToWork({ origin: 'bottom_left' }, 5, 5)).toEqual([5, 5]);
  });
  it('arriba a la izquierda invierte Y', () => {
    expect(workToDoc({ bed, origin: 'top_left' }, [10, 20])).toEqual([10, 280]);
    expect(docDeltaToWork({ origin: 'top_left' }, 5, 5)).toEqual([5, -5]);
  });
  it('a la derecha invierte X y la conversión es su propia inversa', () => {
    const d = { bed, origin: 'top_right' as const };
    expect(workToDoc(d, [10, 20])).toEqual([390, 280]);
    expect(docToWork(d, workToDoc(d, [10, 20]))).toEqual([10, 20]);
    expect(docDeltaToWork(d, 5, 5)).toEqual([-5, -5]);
  });
});

describe('parseGrblVersion', () => {
  it('distingue 1.1f o posterior de las anteriores', () => {
    expect(parseGrblVersion("Grbl 1.1h ['$' for help]")?.modernEnough).toBe(true);
    expect(parseGrblVersion('Grbl 1.1f')?.modernEnough).toBe(true);
    expect(parseGrblVersion('Grbl 1.1e')?.modernEnough).toBe(false);
    expect(parseGrblVersion('Grbl 0.9j')?.modernEnough).toBe(false);
    expect(parseGrblVersion('Grbl 1.2')?.modernEnough).toBe(true);
    expect(parseGrblVersion('Marlin 2.0')).toBeNull();
  });
});
