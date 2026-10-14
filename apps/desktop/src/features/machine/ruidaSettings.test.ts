import { describe, expect, it } from 'vitest';
import type { RuidaSetting } from '@/api/types';
import { CALIBRATABLE, editText, factor, formatValue, groupSettings, parseEdit, pendingChanges, rangeText, toDisplay } from './ruidaSettings';

const s = (over: Partial<RuidaSetting>): RuidaSetting => ({
  key: 'x_max_speed',
  addr: '0x023',
  group: 'axis_x',
  kind: 'speed',
  value: 400000,
  min: 1000,
  max: 600000,
  source_name: 'MEM_MAX_VELOCITY_X',
  ...over,
});

describe('conversión de unidades de Ruida', () => {
  it('velocidades, aceleraciones y longitudes se muestran en mm; el paso y el rotativo, tal cual', () => {
    expect(factor('speed')).toBe(1000);
    expect(factor('accel')).toBe(1000);
    expect(factor('length')).toBe(1000);
    expect(factor('step')).toBe(1);
    expect(factor('raw')).toBe(1);
    expect(toDisplay('speed', 400000)).toBe(400);
    expect(editText('speed', 400000)).toBe('400');
    expect(editText('speed', 12345)).toBe('12.345');
    expect(editText('step', 6350)).toBe('6350');
  });

  it('formatValue quita ceros sobrantes y pone la unidad (el rotativo va sin ella)', () => {
    expect(formatValue('speed', 400000, 'mm/s')).toBe('400 mm/s');
    expect(formatValue('length', 12500, 'mm')).toMatch(/^12[.,]5 mm$/);
    expect(formatValue('step', 6350, 'µm')).toBe('6350 µm');
    expect(formatValue('raw', 5000, 'x')).toBe('5000');
  });

  it('parseEdit acepta coma o punto, redondea a la unidad de la controladora y rechaza lo dudoso', () => {
    const sp = s({});
    expect(parseEdit(sp, '300', 'mm/s')).toEqual({ raw: 300000 });
    expect(parseEdit(sp, '12,5', 'mm/s')).toEqual({ raw: 12500 });
    expect(parseEdit(sp, ' 12.345 ', 'mm/s')).toEqual({ raw: 12345 });
    for (const bad of ['', 'abc', '1e3', '--2', '1.2.3', 'NaN', 'Infinity']) {
      expect(parseEdit(sp, bad, 'mm/s'), bad).toEqual({ error: { key: 'ruida.set.bad.number' } });
    }
    // Más de tres decimales: no cabe en µm/s.
    expect(parseEdit(sp, '12.3456', 'mm/s')).toEqual({ error: { key: 'ruida.set.bad.integer', unit: 'mm/s' } });
    // Fuera de rango, en ambos extremos.
    expect(parseEdit(sp, '0.5', 'mm/s')).toMatchObject({ error: { key: 'ruida.set.bad.range' } });
    expect(parseEdit(sp, '601', 'mm/s')).toMatchObject({ error: { key: 'ruida.set.bad.range', max: expect.any(String) } });
    expect(parseEdit(sp, '1', 'mm/s')).toEqual({ raw: 1000 });
    expect(parseEdit(sp, '600', 'mm/s')).toEqual({ raw: 600000 });
  });

  it('el paso y el rotativo solo admiten enteros', () => {
    const step = s({ key: 'x_step', kind: 'step', min: 1, max: 1000000, value: 6350 });
    expect(parseEdit(step, '6287', 'µm')).toEqual({ raw: 6287 });
    expect(parseEdit(step, '6287.5', 'µm')).toMatchObject({ error: { key: 'ruida.set.bad.integer' } });
  });

  it('con signo: el desplazamiento de home admite negativos', () => {
    const off = s({ key: 'x_home_offset', kind: 'length', min: -1000000, max: 1000000, value: 0 });
    expect(parseEdit(off, '-2.5', 'mm')).toEqual({ raw: -2500 });
    expect(parseEdit(off, '1001', 'mm')).toMatchObject({ error: { key: 'ruida.set.bad.range' } });
  });

  it('rangeText muestra el rango en la unidad mostrada', () => {
    expect(rangeText(s({}), 'mm/s')).toBe('1 – 600 mm/s');
    expect(rangeText(s({ kind: 'length', min: 10000, max: 3000000 }), 'mm')).toBe('10 – 3000 mm');
    expect(rangeText(s({ kind: 'raw', min: 1, max: 100000000 }), '')).toMatch(/^1 – /);
  });
});

describe('cambios pendientes', () => {
  const list = [s({}), s({ key: 'g0_speed', addr: '0x005', group: 'general', value: 200000 }), s({ key: 'x_step', kind: 'step', min: 1, max: 1000000, value: 6350 })];
  const unit = (k: string) => (k === 'step' ? 'µm' : 'mm/s');

  it('solo cuenta los números válidos y distintos del actual; los inválidos dan error; los vacíos se ignoran', () => {
    const { changes, errors } = pendingChanges(list, { x_max_speed: '300', g0_speed: '200', x_step: 'abc' }, unit);
    expect(changes).toEqual([{ key: 'x_max_speed', value: 300000 }]);
    expect(Object.keys(errors)).toEqual(['x_step']);
    expect(pendingChanges(list, { x_max_speed: '  ' }, unit)).toEqual({ changes: [], errors: {} });
  });

  it('agrupa en el orden de la interfaz', () => {
    const groups = groupSettings([s({ key: 'a', group: 'rotary' }), s({ key: 'b', group: 'general' }), s({ key: 'c', group: 'axis_y' }), s({ key: 'd', group: 'otro' })]);
    expect(groups.map(([g]) => g)).toEqual(['general', 'axis_y', 'rotary', 'otro']);
  });

  it('los ejes calibrables son los pasos X, Y, Z y U', () => {
    expect([...CALIBRATABLE]).toEqual(['x_step', 'y_step', 'z_step', 'u_step']);
  });
});
