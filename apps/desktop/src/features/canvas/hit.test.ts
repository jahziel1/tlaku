import { describe, expect, it } from 'vitest';
import { makeLayer } from '@/lib/layers';
import { boxModeFor, hitObject, normalizeBox, pickObject, segmentIntersectsBox, selectByBox } from './hit';
import { getGeom } from './geom';
import { ctxFor, lineObj, rectObj } from './testUtil';

describe('hit-testing', () => {
  const frame = rectObj('a', 10, 10, 50, 40);

  it('un contorno se selecciona por el trazo y no por el interior', () => {
    const ctx = ctxFor([frame]);
    expect(pickObject(ctx, [10, 25], 1)?.id).toBe('a'); // sobre el borde
    expect(pickObject(ctx, [11, 25], 1)?.id).toBe('a'); // dentro de la tolerancia
    expect(pickObject(ctx, [30, 25], 1)).toBeNull(); // interior vacío
    expect(pickObject(ctx, [0, 0], 1)).toBeNull();
  });

  it('una capa de relleno se selecciona también por el interior', () => {
    const ctx = ctxFor([frame], [makeLayer('#ff0000', 'Rojo', { mode: 'fill' })]);
    expect(pickObject(ctx, [30, 25], 1)?.id).toBe('a');
    expect(pickObject(ctx, [60, 25], 1)).toBeNull();
  });

  it('la regla par-impar deja huecos: el agujero de un relleno no se selecciona', () => {
    const ring = rectObj('ring', 0, 0, 100, 100);
    if (ring.kind.type === 'vector') {
      ring.kind.paths.push({ points: [[40, 40], [60, 40], [60, 60], [40, 60]], closed: true });
    }
    const ctx = ctxFor([ring], [makeLayer('#ff0000', 'Rojo', { mode: 'fill' })]);
    expect(pickObject(ctx, [20, 20], 1)?.id).toBe('ring');
    expect(pickObject(ctx, [50, 50], 1)).toBeNull(); // dentro del agujero (lejos de los bordes)
  });

  it('gana el objeto de más arriba (el último del documento)', () => {
    const under = rectObj('under', 0, 0, 20, 20);
    const over = rectObj('over', 0, 0, 20, 20);
    expect(pickObject(ctxFor([under, over]), [0, 10], 1)?.id).toBe('over');
  });

  it('ignora objetos ocultos, bloqueados y de capas invisibles', () => {
    expect(pickObject(ctxFor([rectObj('h', 0, 0, 10, 10, { hidden: true })]), [0, 5], 1)).toBeNull();
    expect(pickObject(ctxFor([rectObj('l', 0, 0, 10, 10, { locked: true })]), [0, 5], 1)).toBeNull();
    const hiddenLayer = [makeLayer('#ff0000', 'Rojo', { visible: false })];
    expect(pickObject(ctxFor([rectObj('v', 0, 0, 10, 10)], hiddenLayer), [0, 5], 1)).toBeNull();
  });

  it('una imagen se selecciona por todo su cuadro, también con transform', () => {
    const img = {
      ...rectObj('img', 0, 0, 1, 1),
      transform: [2, 0, 0, 2, 100, 100] as [number, number, number, number, number, number],
      kind: { type: 'image' as const, source_path: 'a.png', width_px: 10, height_px: 10, size_mm: [10, 10] as [number, number] },
    };
    const ctx = ctxFor([img]);
    expect(pickObject(ctx, [110, 110], 1)?.id).toBe('img'); // centro de la imagen
    expect(pickObject(ctx, [90, 90], 1)).toBeNull();
  });

  it('respeta el transform del objeto', () => {
    const moved = { ...lineObj('l', [0, 0], [10, 0]), transform: [1, 0, 0, 1, 100, 50] as [number, number, number, number, number, number] };
    expect(hitObject(getGeom(moved), 105, 50, 0.5, false)).toBe(true);
    expect(hitObject(getGeom(moved), 5, 0, 0.5, false)).toBe(false);
  });
});

describe('selección por caja', () => {
  const inside = rectObj('inside', 10, 10, 20, 20);
  const crossing = rectObj('crossing', 40, 10, 80, 20); // sale de la caja
  const outside = rectObj('outside', 200, 200, 210, 210);
  const locked = rectObj('locked', 12, 12, 18, 18, { locked: true });
  const hidden = rectObj('hidden', 12, 12, 18, 18, { hidden: true });
  const all = [inside, crossing, outside, locked, hidden];
  const box = normalizeBox([0, 0], [50, 50]);

  it('el sentido del arrastre decide el modo', () => {
    expect(boxModeFor(10, 100)).toBe('window'); // izquierda a derecha
    expect(boxModeFor(100, 10)).toBe('crossing'); // derecha a izquierda
  });

  it('de izquierda a derecha solo toma lo totalmente dentro', () => {
    expect(selectByBox(ctxFor(all), box, 'window')).toEqual(['inside']);
  });

  it('de derecha a izquierda toma también lo que toca', () => {
    expect(selectByBox(ctxFor(all), box, 'crossing')).toEqual(['inside', 'crossing']);
  });

  it('nunca toma bloqueados ni ocultos', () => {
    expect(selectByBox(ctxFor(all), normalizeBox([0, 0], [300, 300]), 'window')).not.toContain('locked');
    expect(selectByBox(ctxFor(all), normalizeBox([0, 0], [300, 300]), 'crossing')).not.toContain('hidden');
  });

  it('"toca" detecta una línea que cruza la caja sin tener vértices dentro', () => {
    const diag = lineObj('diag', [-10, -10], [100, 100]);
    expect(selectByBox(ctxFor([diag]), normalizeBox([20, 30], [30, 40]), 'crossing')).toEqual(['diag']);
    const far = lineObj('far', [-10, 100], [100, 100]);
    expect(selectByBox(ctxFor([far]), normalizeBox([20, 30], [30, 40]), 'crossing')).toEqual([]);
  });

  it('una caja dentro de un relleno toca el objeto aunque no cruce su contorno', () => {
    const solid = rectObj('solid', 0, 0, 100, 100);
    const small = normalizeBox([40, 40], [50, 50]);
    expect(selectByBox(ctxFor([solid]), small, 'crossing')).toEqual([]); // capa de línea: contorno
    const fill = [makeLayer('#ff0000', 'Rojo', { mode: 'fill' })];
    expect(selectByBox(ctxFor([solid], fill), small, 'crossing')).toEqual(['solid']);
  });

  it('segmento contra rectángulo', () => {
    const r: [number, number, number, number] = [0, 0, 10, 10];
    expect(segmentIntersectsBox(-5, 5, 15, 5, r)).toBe(true);
    expect(segmentIntersectsBox(-5, 15, 15, 15, r)).toBe(false);
    expect(segmentIntersectsBox(5, 5, 6, 6, r)).toBe(true);
    expect(segmentIntersectsBox(11, 0, 20, 9, r)).toBe(false);
  });
});
