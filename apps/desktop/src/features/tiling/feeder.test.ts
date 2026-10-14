import { describe, expect, it } from 'vitest';
import { feedLines, formatMm, validateFeeder, type FeederSettings } from './feeder';

const base: FeederSettings = { enabled: true, feed_gcode: 'G91\nG0 Y{mm}\nG90', feed_mm: 300 };

describe('feedLines', () => {
  it('sustituye {mm} y separa las órdenes', () => {
    expect(feedLines(base, 70)).toEqual(['G91', 'G0 Y70', 'G90']);
  });
  it('quita comentarios, líneas vacías y ceros sobrantes', () => {
    const f = { ...base, feed_gcode: '; avanza\nG91 ; relativo\n\nG0 Y{mm} (cinta)\r\nG90' };
    expect(feedLines(f, 69.5)).toEqual(['G91', 'G0 Y69.5', 'G90']);
    expect(formatMm(300)).toBe('300');
    expect(formatMm(12.34567)).toBe('12.346');
  });
});

describe('validateFeeder', () => {
  it('apagado no se valida; el de fábrica es válido', () => {
    expect(validateFeeder({ ...base, enabled: false, feed_gcode: '', feed_mm: -1 })).toEqual({});
    expect(validateFeeder(base)).toEqual({});
  });
  it('exige G-code con {mm} y sin otros marcadores', () => {
    expect(validateFeeder({ ...base, feed_gcode: '  ' }).feed_gcode).toBe('gcodeEmpty');
    expect(validateFeeder({ ...base, feed_gcode: 'G0 Y10' }).feed_gcode).toBe('gcodeNoMm');
    expect(validateFeeder({ ...base, feed_gcode: 'G0 Y{mm} F{speed}' }).feed_gcode).toBe('gcodePlaceholder');
    expect(validateFeeder({ ...base, feed_gcode: `G0 Y{mm} ${'X'.repeat(130)}` }).feed_gcode).toBe('gcodeLong');
  });
  it('el avance es un número positivo y razonable', () => {
    expect(validateFeeder({ ...base, feed_mm: 0 }).feed_mm).toBe('mmPositive');
    expect(validateFeeder({ ...base, feed_mm: Number.NaN }).feed_mm).toBe('mmPositive');
    expect(validateFeeder({ ...base, feed_mm: 6000 }).feed_mm).toBe('mmTooBig');
  });
});
