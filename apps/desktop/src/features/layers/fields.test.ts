import { describe, expect, it } from 'vitest';
import { dpiToInterval } from '@/lib/layers';
import { FIELD_SPECS, formatNumber, parseDecimal, validateField, type NumericField } from './fields';

const ok = (field: NumericField, text: string, ctx = {}) => {
  const r = validateField(field, text, ctx);
  return r.ok ? r.value : r.error.key;
};

describe('parseDecimal', () => {
  it('acepta coma o punto y rechaza lo que no es un número', () => {
    expect(parseDecimal('1,5')).toBe(1.5);
    expect(parseDecimal(' 2.25 ')).toBe(2.25);
    expect(parseDecimal('-3')).toBe(-3);
    for (const bad of ['', '  ', 'abc', '12abc', '1,2,3', 'Infinity', 'NaN']) expect(parseDecimal(bad), bad).toBeNull();
  });
});

describe('validateField', () => {
  it('texto que no es un número', () => {
    expect(ok('speed', '')).toBe('layers.err.number');
    expect(ok('speed', 'rápido')).toBe('layers.err.number');
  });

  it('rangos de velocidad y potencia', () => {
    expect(ok('speed', '1000')).toBe(1000);
    expect(ok('speed', '0')).toBe('layers.err.range');
    expect(ok('speed', '60001')).toBe('layers.err.range');
    expect(ok('power', '0')).toBe(0);
    expect(ok('power', '100')).toBe(100);
    expect(ok('power', '100,5')).toBe('layers.err.range');
    expect(ok('power', '-1')).toBe('layers.err.range');
  });

  it('el mensaje de rango dice los límites y la unidad', () => {
    const r = validateField('power', '120');
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.error.params).toEqual({ min: '0', max: '100', unit: '%' });
  });

  it('pasadas y posición solo admiten enteros', () => {
    expect(ok('passes', '3')).toBe(3);
    expect(ok('passes', '2.5')).toBe('layers.err.integer');
    expect(ok('passes', '0')).toBe('layers.err.range');
    expect(ok('passes', '1001')).toBe('layers.err.range');
    expect(ok('order', '1.5')).toBe('layers.err.integer');
  });

  it('la posición fijada no puede pasar del número de capas', () => {
    expect(ok('order', '3', { layerCount: 3 })).toBe(3);
    expect(ok('order', '4', { layerCount: 3 })).toBe('layers.err.range');
    expect(ok('order', '0', { layerCount: 3 })).toBe('layers.err.range');
  });

  it('la potencia mínima no supera la máxima, y la máxima no baja de la mínima', () => {
    expect(ok('min_power', '10', { power: 40 })).toBe(10);
    expect(ok('min_power', '40', { power: 40 })).toBe(40);
    expect(ok('min_power', '41', { power: 40 })).toBe('layers.err.minAbovePower');
    expect(ok('power', '20', { minPower: 30 })).toBe('layers.err.powerBelowMin');
    expect(ok('power', '30', { minPower: 30 })).toBe(30);
  });

  it('intervalo, ángulo, kerf y overscan', () => {
    expect(ok('interval', '0,1')).toBe(0.1);
    expect(ok('interval', '0')).toBe('layers.err.range');
    expect(ok('interval', '5.1')).toBe('layers.err.range');
    expect(ok('angle', '-45')).toBe(-45);
    expect(ok('angle', '361')).toBe('layers.err.range');
    expect(ok('kerf', '-0.15')).toBe(-0.15);
    expect(ok('kerf', '6')).toBe('layers.err.range');
    expect(ok('overscan', '0')).toBe(0);
    expect(ok('overscan', '-1')).toBe('layers.err.range');
  });

  it('ajustes de imagen', () => {
    expect(ok('threshold', '0.5')).toBe(0.5);
    expect(ok('threshold', '128')).toBe('layers.err.range'); // valor en escala 0-255: el motor lo rechazaría
    expect(ok('brightness', '-1')).toBe(-1);
    expect(ok('brightness', '1.2')).toBe('layers.err.range');
    expect(ok('contrast', '0.3')).toBe(0.3);
    expect(ok('gamma', '0')).toBe('layers.err.range');
    expect(ok('gamma', '2.2')).toBe(2.2);
    expect(ok('dot_width', '0')).toBe(0);
    expect(ok('dot_width', '-0.1')).toBe('layers.err.range');
  });

  it('el rango de DPI corresponde al rango de intervalo', () => {
    const { min, max } = FIELD_SPECS.dpi;
    expect(dpiToInterval(max)).toBeCloseTo(FIELD_SPECS.interval.min, 9);
    expect(dpiToInterval(min)).toBeCloseTo(FIELD_SPECS.interval.max, 9);
    expect(ok('dpi', '300')).toBe(300);
    expect(ok('dpi', '2541')).toBe('layers.err.range');
    expect(ok('dpi', '5')).toBe('layers.err.range');
  });
});

describe('formatNumber', () => {
  it('quita ceros sobrantes y ruido de coma flotante', () => {
    expect(formatNumber(0.30000000000000004, 4)).toBe('0.3');
    expect(formatNumber(1000, 0)).toBe('1000');
    expect(formatNumber(2.5, 2)).toBe('2.5');
    expect(formatNumber(25.4 / 0.1, 1)).toBe('254');
  });
});

describe('campos de la Fase 4', () => {
  it('ancho de puente y celda deben ser mayores que cero', () => {
    expect(ok('tab_width', '0.5')).toBe(0.5);
    expect(ok('tab_width', '0')).toBe('layers.err.positive');
    expect(ok('tab_width', '-1')).toBe('layers.err.positive');
    expect(ok('tab_width', '51')).toBe('layers.err.range');
    expect(ok('halftone_cell', '0')).toBe('layers.err.positive');
    expect(ok('halftone_cell', '0,5')).toBe(0.5);
    expect(ok('halftone_cell', '11')).toBe('layers.err.range');
  });

  it('cantidad de puentes es entera y al menos 1', () => {
    expect(ok('tab_count', '4')).toBe(4);
    expect(ok('tab_count', '2.5')).toBe('layers.err.integer');
    expect(ok('tab_count', '0')).toBe('layers.err.range');
  });

  it('separación: 0 usa la cantidad, y si no debe superar el ancho del puente', () => {
    expect(ok('tab_spacing', '0', { tabWidth: 2 })).toBe(0);
    expect(ok('tab_spacing', '10', { tabWidth: 2 })).toBe(10);
    expect(ok('tab_spacing', '2', { tabWidth: 2 })).toBe('layers.err.spacingBelowWidth');
    expect(ok('tab_spacing', '1', { tabWidth: 2 })).toBe('layers.err.spacingBelowWidth');
    expect(ok('tab_spacing', '-1')).toBe('layers.err.range');
  });

  it('ángulo de la rejilla y Z', () => {
    expect(ok('halftone_angle', '45')).toBe(45);
    expect(ok('halftone_angle', '400')).toBe('layers.err.range');
    expect(ok('z_offset', '-2.5')).toBe(-2.5);
    expect(ok('z_offset', '101')).toBe('layers.err.range');
    expect(ok('z_step', '0.2')).toBe(0.2);
    expect(ok('z_step', 'x')).toBe('layers.err.number');
  });
});
