import { describe, expect, it } from 'vitest';
import { fitBounds, formatTick, gridSteps, MAX_ZOOM, MIN_ZOOM, zoomAround, zoomPercent } from './viewport';

describe('viewport', () => {
  it('fitBounds centra y deja margen', () => {
    const v = fitBounds([[0, 0], [400, 400]], 1000, 800, 40);
    expect(400 * v.zoom).toBeLessThanOrEqual(720 + 1e-6);
    expect(v.panX + 200 * v.zoom).toBeCloseTo(500, 6);
    expect(v.panY - 200 * v.zoom).toBeCloseTo(400, 6);
  });
  it('limita el zoom', () => {
    expect(zoomAround({ zoom: 1, panX: 0, panY: 0 }, 1e9, 0, 0).zoom).toBe(MAX_ZOOM);
    expect(zoomAround({ zoom: 1, panX: 0, panY: 0 }, 1e-9, 0, 0).zoom).toBe(MIN_ZOOM);
  });
  it('la cuadrícula se adapta al zoom y 100 % es tamaño real', () => {
    expect(gridSteps(0.1).minor).toBeGreaterThanOrEqual(50);
    expect(gridSteps(20).minor).toBeLessThanOrEqual(1);
    expect(gridSteps(2).major).toBe(gridSteps(2).minor * 10);
    expect(zoomPercent(96 / 25.4)).toBe(100);
  });
  it('formatea las cotas de la regla', () => {
    expect(formatTick(50)).toBe('50');
    expect(formatTick(0.5)).toBe('0.5');
  });
});
