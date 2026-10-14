import { afterEach, describe, expect, it, vi } from 'vitest';
import { createAppStore } from '@/store';
import { createShapeGesture } from './shapeGesture';
import { GRID_CONTRAST_FACTOR, readCanvasPalette, scaleAlpha } from './palette';

describe('contraste de la rejilla', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('multiplica la opacidad sin pasar de 1 y deja intactos los colores que no son rgba', () => {
    expect(scaleAlpha('rgba(165, 190, 225, 0.085)', 2)).toBe('rgba(165, 190, 225, 0.17)');
    expect(scaleAlpha('rgba(165, 190, 225, 0.6)', 3)).toBe('rgba(165, 190, 225, 1)');
    expect(scaleAlpha('rgba(1, 2, 3, 0.5)', 1)).toBe('rgba(1, 2, 3, 0.5)');
    expect(scaleAlpha('#808080', 2)).toBe('#808080');
  });

  it('la paleta aplica el contraste solo a las líneas de la rejilla', () => {
    vi.stubGlobal('getComputedStyle', () => ({
      getPropertyValue: (name: string) =>
        name === '--cv-grid-minor' ? 'rgba(165, 190, 225, 0.1)' : name === '--cv-grid-major' ? 'rgba(165, 190, 225, 0.2)' : name === '--cv-bed' ? '#151c26' : '#fff',
    }));
    const el = {} as Element;
    const normal = readCanvasPalette(el);
    expect(normal.gridMinor).toBe('rgba(165, 190, 225, 0.1)');
    const high = readCanvasPalette(el, 'high');
    expect(high.gridMinor).toBe(`rgba(165, 190, 225, ${Math.round(0.1 * GRID_CONTRAST_FACTOR.high * 1000) / 1000})`);
    expect(high.gridMajor).not.toBe(normal.gridMajor);
    expect(high.bed).toBe(normal.bed);
    const low = readCanvasPalette(el, 'low');
    expect(Number(/, ([\d.]+)\)$/.exec(low.gridMinor)![1])).toBeLessThan(0.1);
  });
});

describe('volver a Seleccionar tras dibujar', () => {
  function draw(returnToSelect: boolean) {
    const store = createAppStore();
    store.getState().setPref('returnToSelect', returnToSelect);
    store.getState().setTool('rect');
    const ctx = { st: () => store.getState(), state: {} } as never;
    const { finishDraw } = createShapeGesture(ctx);
    finishDraw({ kind: 'draw', tool: 'rect', active: true, startWorld: [10, 10], end: [40, 30] } as never, { shift: false } as never);
    return store;
  }

  it('encendido (de fábrica): crea la forma, la selecciona y pasa a Seleccionar', () => {
    const store = draw(true);
    expect(store.getState().doc.objects).toHaveLength(1);
    expect(store.getState().selectedIds).toHaveLength(1);
    expect(store.getState().activeTool).toBe('select');
  });

  it('apagado: la herramienta sigue activa para dibujar otra', () => {
    const store = draw(false);
    expect(store.getState().doc.objects).toHaveLength(1);
    expect(store.getState().selectedIds).toHaveLength(1);
    expect(store.getState().activeTool).toBe('rect');
  });
});
