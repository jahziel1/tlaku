import { describe, expect, it } from 'vitest';
import type { JobPreview, LayerSettings } from '@/api/types';
import { makeLayer } from '@/lib/layers';
import { remainingSeconds, summarizeJob } from './jobSummary';

const layers: LayerSettings[] = [
  makeLayer('#ff0000', 'Corte', { power: 85, speed: 400, passes: 2 }),
  makeLayer('#0000ff', 'Grabado', { mode: 'fill', power: 30, speed: 3000 }),
];
const preview = (over: Partial<JobPreview> = {}): JobPreview =>
  ({
    layers: [
      { color: '#FF0000', mode: 'line', seconds: 60, summary: '', travel_saved_pct: null, explanations: [] },
      { color: '#0000ff', mode: 'fill', seconds: 100, summary: '', travel_saved_pct: null, explanations: [] },
      { color: '#0000ff', mode: 'fill', seconds: 20, summary: '', travel_saved_pct: null, explanations: [] },
    ],
    moves: [],
    move_times: [],
    total_s: 180,
    bounds: [[0, 0], [10, 10]],
    order_note: null,
    warnings: ['aviso'],
    ...over,
  }) as JobPreview;

describe('summarizeJob', () => {
  it('junta las entradas de la misma capa y toma potencia y velocidad de la capa del documento', () => {
    const s = summarizeJob({ layers }, preview());
    expect(s.layers).toHaveLength(2);
    expect(s.layers[0]).toMatchObject({ name: 'Corte', power: 85, speed: 400, passes: 2, seconds: 60 });
    expect(s.layers[1]).toMatchObject({ name: 'Grabado', seconds: 120 });
    expect(s.totalSeconds).toBe(180);
    expect(s.maxPower).toBe(85);
    expect(s.warnings).toEqual(['aviso']);
  });

  it('la potencia máxima sale solo de las capas que realmente se envían', () => {
    const only = preview({ layers: [preview().layers[1]!] });
    expect(summarizeJob({ layers }, only).maxPower).toBe(30);
  });

  it('una capa que ya no existe en el documento no rompe el resumen', () => {
    const s = summarizeJob({ layers: [] }, preview());
    expect(s.layers[0]).toMatchObject({ name: '#FF0000', power: 0 });
    expect(s.maxPower).toBe(0);
  });
});

describe('remainingSeconds', () => {
  it('al principio usa la estimación del motor', () => {
    expect(remainingSeconds(0, 100, 0)).toBe(100);
    expect(remainingSeconds(0.01, 100, 1)).toBeCloseTo(99, 6);
  });

  it('con avance suficiente mezcla el ritmo real con la estimación', () => {
    // 50 % en 30 s -> faltan 30 s por ritmo; estimación 100 s -> faltan 50 s.
    expect(remainingSeconds(0.5, 100, 30)).toBeCloseTo(0.7 * 30 + 0.3 * 50, 6);
  });

  it('sin estimación usa solo el ritmo y, sin ninguno, no sabe', () => {
    expect(remainingSeconds(0.5, null, 30)).toBeCloseTo(30, 6);
    expect(remainingSeconds(0, null, 0)).toBeNull();
  });

  it('al terminar no queda nada', () => {
    expect(remainingSeconds(1, 100, 100)).toBe(0);
  });
});
