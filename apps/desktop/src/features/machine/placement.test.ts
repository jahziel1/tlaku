import { describe, expect, it } from 'vitest';
import type { DesignObject, LayerSettings } from '@/api/types';
import { makeLayer } from '@/lib/layers';
import { translation } from '@/lib/geometry';
import { anchorFraction, anchorPoint, computeJobPlacement, designBounds, type PlacementInput } from './placement';
import { polylineBez } from '@/lib/bezpath';

const bedDevice = { bed: [400, 300] as [number, number], origin: 'bottom_left' as const };
/** Diseño de 100 x 50 mm con la esquina inferior izquierda en (60, 70). */
const design: PlacementInput['design'] = [[60, 70], [160, 120]];

const base: PlacementInput = {
  startFrom: 'absolute',
  jobOrigin: 6,
  userOrigin: null,
  head: null,
  device: bedDevice,
  design,
};

describe('los 9 puntos', () => {
  it('van fila por fila desde arriba a la izquierda', () => {
    expect(anchorFraction(0)).toEqual([0, 0]);
    expect(anchorFraction(2)).toEqual([1, 0]);
    expect(anchorFraction(4)).toEqual([0.5, 0.5]);
    expect(anchorFraction(6)).toEqual([0, 1]);
    expect(anchorFraction(8)).toEqual([1, 1]);
  });

  it('0 = arriba-izquierda, 4 = centro y 6 = abajo-izquierda sobre el rectángulo (Y hacia arriba)', () => {
    expect(anchorPoint(design!, 0)).toEqual([60, 120]);
    expect(anchorPoint(design!, 4)).toEqual([110, 95]);
    expect(anchorPoint(design!, 6)).toEqual([60, 70]);
    expect(anchorPoint(design!, 8)).toEqual([160, 70]);
  });

  it('un índice fuera de rango o no entero se acota al centro o al borde', () => {
    expect(anchorFraction(99)).toEqual([1, 1]);
    expect(anchorFraction(-3)).toEqual([0, 0]);
    expect(anchorFraction(1.5)).toEqual([0.5, 0.5]);
  });
});

describe('computeJobPlacement', () => {
  it('absolutas: el rectángulo queda tal cual y no hay punto de anclaje', () => {
    const p = computeJobPlacement(base);
    expect(p.status).toBe('ok');
    expect(p.rect).toEqual(design);
    expect(p.anchor).toBeNull();
    expect(p.target).toBeNull();
    expect(p.fits).toBe(true);
  });

  it('posición actual: el punto elegido coincide con el cabezal', () => {
    const p = computeJobPlacement({ ...base, startFrom: 'current_position', jobOrigin: 4, head: [200, 150] });
    expect(p.rect).toEqual([[150, 125], [250, 175]]);
    expect(p.anchor).toEqual([200, 150]);
    expect(p.fits).toBe(true);
  });

  it('abajo-izquierda (6) pone la esquina del trabajo en el origen de usuario', () => {
    const p = computeJobPlacement({ ...base, startFrom: 'user_origin', jobOrigin: 6, userOrigin: [10, 10] });
    expect(p.rect).toEqual([[10, 10], [110, 60]]);
  });

  it('arriba-derecha (2) cuelga el trabajo hacia abajo y a la izquierda del punto', () => {
    const p = computeJobPlacement({ ...base, startFrom: 'current_position', jobOrigin: 2, head: [300, 250] });
    expect(p.rect).toEqual([[200, 200], [300, 250]]);
  });

  it('Ruida: la controladora resuelve la posición actual y el origen de usuario al arrancar', () => {
    // Ni el cabezal de ahora ni el origen de la aplicación cuentan, y no bloquea el inicio.
    for (const startFrom of ['current_position', 'user_origin'] as const) {
      const p = computeJobPlacement({ ...base, startFrom, head: [350, 10], userOrigin: [390, 10], controllerResolves: true });
      expect(p.status).toBe('deferred');
      expect(p.rect).toBeNull();
      expect(p.fits).toBe(true);
    }
    // Sin cabezal ni origen de la aplicación tampoco se pide nada.
    expect(computeJobPlacement({ ...base, startFrom: 'current_position', controllerResolves: true }).status).toBe('deferred');
    // Absoluto sigue colocándose en la cama; sin diseño no hay nada que enviar.
    expect(computeJobPlacement({ ...base, controllerResolves: true }).status).toBe('ok');
    expect(computeJobPlacement({ ...base, startFrom: 'user_origin', design: null, controllerResolves: true }).status).toBe('noDesign');
  });

  it('avisa por cuántos milímetros y por qué lado se sale de la cama', () => {
    const p = computeJobPlacement({ ...base, startFrom: 'current_position', jobOrigin: 6, head: [350, 10] });
    expect(p.fits).toBe(false);
    expect(p.overflow.right).toBeCloseTo(50, 6);
    expect(p.overflow.left).toBe(0);
    const q = computeJobPlacement({ ...base, startFrom: 'current_position', jobOrigin: 4, head: [20, 290] });
    expect(q.overflow.left).toBeCloseTo(30, 6);
    expect(q.overflow.top).toBeCloseTo(15, 6);
  });

  it('pide lo que falta en vez de inventar un punto', () => {
    expect(computeJobPlacement({ ...base, startFrom: 'current_position' }).status).toBe('needHead');
    expect(computeJobPlacement({ ...base, startFrom: 'user_origin' }).status).toBe('needUserOrigin');
    expect(computeJobPlacement({ ...base, design: null }).status).toBe('noDesign');
    expect(computeJobPlacement({ ...base, startFrom: 'user_origin' }).rect).toBeNull();
  });

  it('con origen arriba a la izquierda el cabezal se convierte al plano del documento', () => {
    const device = { bed: [400, 300] as [number, number], origin: 'top_left' as const };
    // Y de máquina 20 desde arriba = Y de documento 280.
    const p = computeJobPlacement({ ...base, device, startFrom: 'current_position', jobOrigin: 0, head: [50, 20] });
    expect(p.target).toEqual([50, 280]);
    expect(p.rect).toEqual([[50, 230], [150, 280]]);
  });
});

describe('designBounds', () => {
  const layer = (color: string, output = true): LayerSettings => makeLayer(color, color, { output });
  const obj = (id: string, layerColor: string, x: number, hidden = false): DesignObject => ({
    id,
    layer: layerColor,
    transform: translation(x, 0),
    locked: false,
    hidden,
    group: null,
    kind: { type: 'vector', paths: [polylineBez([[0, 0], [10, 0], [10, 10], [0, 10]], true)] },
  });
  const doc = {
    layers: [layer('#ff0000'), layer('#00ff00', false)],
    objects: [obj('a', '#ff0000', 0), obj('b', '#FF0000', 100), obj('c', '#00ff00', 500), obj('d', '#ff0000', 900, true)],
  };

  it('une solo lo que sale: capas con salida y objetos visibles', () => {
    expect(designBounds(doc, null)).toEqual([[0, 0], [110, 10]]);
  });

  it('respeta la selección del trabajo', () => {
    expect(designBounds(doc, ['b'])).toEqual([[100, 0], [110, 10]]);
    expect(designBounds(doc, ['c', 'd'])).toBeNull();
  });
});

describe('solo la selección: caja de anclaje', () => {
  // Selección de 20 x 10 dentro de un diseño de 100 x 50; el cabezal está en (200, 100).
  const selection: PlacementInput['design'] = [[10, 10], [30, 20]];
  const whole: PlacementInput['design'] = [[0, 0], [100, 50]];
  const input: PlacementInput = { ...base, startFrom: 'current_position', head: [200, 100], design: selection };

  it('con el origen de la selección, la esquina de la selección queda en el cabezal', () => {
    const p = computeJobPlacement({ ...input, originBox: selection });
    expect(p.rect).toEqual([[200, 100], [220, 110]]);
  });

  it('sin él, la esquina de todo el diseño queda en el cabezal y la selección conserva su lugar', () => {
    const p = computeJobPlacement({ ...input, originBox: whole });
    expect(p.rect).toEqual([[210, 110], [230, 120]]);
  });

  it('sin caja de anclaje aparte se usa la del diseño enviado', () => {
    expect(computeJobPlacement(input).rect).toEqual(computeJobPlacement({ ...input, originBox: selection }).rect);
  });

  it('en coordenadas absolutas no cambia nada', () => {
    expect(computeJobPlacement({ ...input, startFrom: 'absolute', originBox: whole }).rect).toEqual(selection);
  });
});
