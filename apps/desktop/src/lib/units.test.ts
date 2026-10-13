import { describe, expect, it } from 'vitest';
import { formatLength, fromDisplay, parseLength, toDisplay } from './units';

describe('conversión de unidades', () => {
  it('25,4 mm son una pulgada', () => {
    expect(toDisplay(25.4, 'in')).toBeCloseTo(1, 12);
    expect(fromDisplay(1, 'in')).toBeCloseTo(25.4, 12);
    expect(toDisplay(12.5, 'mm')).toBe(12.5);
    expect(fromDisplay(12.5, 'mm')).toBe(12.5);
  });
});

describe('formatLength', () => {
  it('usa coma decimal y dos decimales en mm', () => {
    expect(formatLength(12.5)).toBe('12,50 mm');
    expect(formatLength(12.5, 'mm', { symbol: false })).toBe('12,50');
    expect(formatLength(0, 'mm')).toBe('0,00 mm');
  });
  it('en pulgadas usa tres decimales', () => {
    expect(formatLength(25.4, 'in')).toBe('1,000 in');
    expect(formatLength(10, 'in')).toBe('0,394 in');
  });
  it('quita ceros sobrantes con trim', () => {
    expect(formatLength(12.5, 'mm', { trim: true })).toBe('12,5 mm');
    expect(formatLength(10, 'mm', { trim: true, symbol: false })).toBe('10');
    expect(formatLength(25.4, 'in', { trim: true, symbol: false })).toBe('1');
  });
  it('respeta los decimales pedidos y no muestra -0', () => {
    expect(formatLength(1.23456, 'mm', { digits: 3 })).toBe('1,235 mm');
    expect(formatLength(-0.001, 'mm', { symbol: false })).toBe('0,00');
    expect(formatLength(-5, 'mm', { symbol: false })).toBe('-5,00');
  });
  it('un número no finito se muestra como raya', () => {
    expect(formatLength(Number.NaN)).toBe('—');
    expect(formatLength(Infinity, 'in')).toBe('—');
  });
});

describe('parseLength', () => {
  it('acepta coma o punto decimal', () => {
    expect(parseLength('12,5')).toBe(12.5);
    expect(parseLength('12.5')).toBe(12.5);
    expect(parseLength('  7 ')).toBe(7);
    expect(parseLength('.5')).toBe(0.5);
  });
  it('un sufijo manda sobre la unidad por omisión', () => {
    expect(parseLength('30mm', 'in')).toBe(30);
    expect(parseLength('2 in')).toBeCloseTo(50.8, 10);
    expect(parseLength('2"')).toBeCloseTo(50.8, 10);
    expect(parseLength('2 pulg')).toBeCloseTo(50.8, 10);
    expect(parseLength('5 cm')).toBe(50);
    expect(parseLength('0,25 IN')).toBeCloseTo(6.35, 10);
  });
  it('sin sufijo usa la unidad por omisión', () => {
    expect(parseLength('1', 'in')).toBeCloseTo(25.4, 10);
    expect(parseLength('1', 'mm')).toBe(1);
  });
  it('entiende fracciones en pulgadas', () => {
    expect(parseLength('1/2', 'in')).toBeCloseTo(12.7, 10);
    expect(parseLength('1 1/2 in')).toBeCloseTo(38.1, 10);
    expect(parseLength('3/8"')).toBeCloseTo(9.525, 10);
    expect(parseLength('1/0', 'in')).toBeNull();
    // En milímetros una fracción no tiene sentido.
    expect(parseLength('1/2', 'mm')).toBeNull();
  });
  it('acepta negativos', () => {
    expect(parseLength('-3,5')).toBe(-3.5);
    expect(parseLength('- 1 in')).toBeCloseTo(-25.4, 10);
  });
  it('rechaza lo que no es una medida', () => {
    for (const bad of ['', '  ', 'abc', 'mm', '12 kg', '1,2,3', '1e3', '--2', '12 mm mm', '1/']) {
      expect(parseLength(bad), bad).toBeNull();
    }
  });
  it('lo que muestra formatLength se vuelve a leer igual', () => {
    for (const mm of [0, 0.1, 1, 12.5, 100, 254]) {
      expect(parseLength(formatLength(mm, 'mm'))).toBeCloseTo(mm, 2);
      expect(parseLength(formatLength(mm, 'in'))).toBeCloseTo(mm, 1);
    }
  });
});

describe('campos de longitud', () => {
  it('fieldText deja el mm tal cual y redondea las pulgadas a diezmilésimas', async () => {
    const { fieldText } = await import('./units');
    expect(fieldText(12.3456, 'mm')).toBe('12.3456');
    expect(fieldText(25.4, 'in')).toBe('1');
    expect(fieldText(10, 'in')).toBe('0.3937');
    expect(fieldText(-0.001, 'in')).toBe('0');
  });
  it('stepInMm redondea el paso a uno bonito en pulgadas', async () => {
    const { stepInMm } = await import('./units');
    expect(stepInMm(0.5, 'mm')).toBe(0.5);
    // 0,5 mm = 0,0197 in -> 0,02 in; 1 mm = 0,039 in -> 0,05 in; 25 mm -> 1 in
    expect(stepInMm(0.5, 'in') / 25.4).toBeCloseTo(0.02, 9);
    expect(stepInMm(1, 'in') / 25.4).toBeCloseTo(0.05, 9);
    expect(stepInMm(25, 'in') / 25.4).toBeCloseTo(1, 9);
  });
  it('formatSize une ancho, alto y unidad', async () => {
    const { formatSize } = await import('./units');
    expect(formatSize(40, 20.55, 'mm')).toBe('40,0 × 20,6 mm');
    expect(formatSize(400, 415, 'mm', 0)).toBe('400 × 415 mm');
    expect(formatSize(25.4, 50.8, 'in', 1)).toBe('1,00 × 2,00 in');
    expect(formatSize(400, 415, 'in', 0)).toBe('15,7 × 16,3 in');
  });
});
