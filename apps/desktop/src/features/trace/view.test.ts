import { describe, expect, it } from 'vitest';
import { fitView, MAX_ZOOM, panBy, viewBoxOf, zoomAt } from './view';

describe('vista previa', () => {
  it('ver completa abarca toda la imagen', () => {
    expect(viewBoxOf(fitView(80, 50), 80, 50)).toBe('0 0 80 50');
  });

  it('acercar mantiene fijo el punto bajo el cursor', () => {
    const v = zoomAt(fitView(80, 50), 2, 60, 10, 80, 50);
    const w = 80 / v.zoom;
    const x0 = v.cx - w / 2;
    // el punto (60, 10) sigue en la misma fracción de la ventana: (60-0)/80
    expect((60 - x0) / w).toBeCloseTo(60 / 80, 6);
  });

  it('no se aleja más que la imagen completa ni se acerca sin tope, y no se sale de la imagen', () => {
    expect(zoomAt(fitView(80, 50), 0.1, 10, 10, 80, 50).zoom).toBe(1);
    expect(zoomAt(fitView(80, 50), 1e6, 10, 10, 80, 50).zoom).toBe(MAX_ZOOM);
    const v = panBy(zoomAt(fitView(80, 50), 4, 40, 25, 80, 50), 1000, -1000, 80, 50);
    expect(v.cx).toBe(80 - 80 / 4 / 2);
    expect(v.cy).toBe(50 / 4 / 2);
  });
});
