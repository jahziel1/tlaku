import { describe, expect, it } from 'vitest';
import { createMockApi } from '@/api/mock';
import { bezBounds, type BezPath } from '@/lib/bezpath';
import { DEFAULT_TRACE_SETTINGS } from './settings';
import { looksRough, traceStats } from './stats';

const api = createMockApi({ latencyMs: 0 });
const size: [number, number] = [80, 50];
const trace = (patch = {}) =>
  api.traceImage({ path: '/tmp/a.png', settings: { ...DEFAULT_TRACE_SETTINGS, ...patch }, size_mm: size });

describe('traceImage simulado', () => {
  it('devuelve formas dentro del tamaño pedido', async () => {
    const { paths } = await trace();
    expect(paths.length).toBeGreaterThanOrEqual(3);
    const [[x0, y0], [x1, y1]] = bezBounds(paths)!;
    expect(x0).toBeGreaterThanOrEqual(0);
    expect(y0).toBeGreaterThanOrEqual(0);
    expect(x1).toBeLessThanOrEqual(80);
    expect(y1).toBeLessThanOrEqual(50);
    api.dispose();
  });

  it('el umbral cambia visiblemente el resultado', async () => {
    const low = bezBounds((await trace({ threshold: 0.2 })).paths)!;
    const high = bezBounds((await trace({ threshold: 0.8 })).paths)!;
    const area = ([[a, b], [c, d]]: [[number, number], [number, number]]) => (c - a) * (d - b);
    expect(area(high)).toBeGreaterThan(area(low));
  });

  it('el modo polígono no tiene curvas y menos suavizado da más nodos', async () => {
    const spline = await trace();
    expect(spline.paths.flat().some((s) => s.type === 'cubic_to')).toBe(true);
    const rough = await trace({ mode: 'polygon', smoothing: 0 });
    const smooth = await trace({ mode: 'polygon', smoothing: 1 });
    expect(rough.paths.flat().some((s) => s.type === 'cubic_to')).toBe(false);
    expect(traceStats(rough.paths).nodes).toBeGreaterThan(traceStats(smooth.paths).nodes);
  });

  it('eliminar manchas descarta las motas y avisa', async () => {
    const withSpecks = await trace({ speckle_px: 0 });
    const clean = await trace({ speckle_px: 8 });
    expect(withSpecks.paths.length).toBeGreaterThan(clean.paths.length);
    expect(clean.warnings.length).toBeGreaterThan(0);
  });

  it('invertir da una sola forma con huecos', async () => {
    const { paths } = await trace({ invert: true });
    expect(paths).toHaveLength(1);
    expect(paths[0]!.filter((s) => s.type === 'move_to').length).toBeGreaterThan(1);
  });

  it('rechaza una ruta vacía', async () => {
    await expect(api.traceImage({ path: '', settings: DEFAULT_TRACE_SETTINGS, size_mm: size })).rejects.toThrow();
  });
});

describe('stats', () => {
  it('cuenta formas y nodos de anclaje y detecta lo áspero', () => {
    const square: BezPath = [
      { type: 'move_to', to: [0, 0] },
      { type: 'line_to', to: [1, 0] },
      { type: 'cubic_to', ctrl1: [1, 1], ctrl2: [0, 1], to: [0, 0] },
      { type: 'close' },
    ];
    expect(traceStats([square, square])).toEqual({ shapes: 2, nodes: 6 });
    expect(looksRough({ shapes: 5, nodes: 100 })).toBe(false);
    expect(looksRough({ shapes: 5000, nodes: 100 })).toBe(true);
    expect(looksRough({ shapes: 5, nodes: 90000 })).toBe(true);
  });
});
