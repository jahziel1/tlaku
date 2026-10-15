import { describe, expect, it } from 'vitest';
import { contains, DETAIL_MAX_PX, intersect, planDetail, visibleBox, type DetailPlanInput } from './plan';

const sheet: [number, number, number, number] = [0, 0, 100, 100];

function input(patch: Partial<DetailPlanInput> = {}): DetailPlanInput {
  return { view: { zoom: 4, panX: 0, panY: 400 }, width: 400, height: 400, dpr: 1, sheet, fullPxMm: 0.1, detail: null, ...patch };
}

describe('zona visible', () => {
  it('pasa de píxeles de pantalla a mm del documento con Y hacia arriba', () => {
    expect(visibleBox({ zoom: 4, panX: 0, panY: 400 }, 400, 400)).toEqual([0, 0, 100, 100]);
    expect(visibleBox({ zoom: 2, panX: 100, panY: 300 }, 200, 100)).toEqual([-50, 100, 50, 150]);
  });
  it('intersecta y contiene', () => {
    expect(intersect([0, 0, 10, 10], [5, 5, 20, 20])).toEqual([5, 5, 10, 10]);
    expect(intersect([0, 0, 10, 10], [10, 0, 20, 10])).toBeNull();
    expect(contains([0, 0, 10, 10], [1, 1, 9, 9])).toBe(true);
    expect(contains([0, 0, 10, 10], [1, 1, 11, 9])).toBe(false);
  });
});

describe('detalle según el zoom', () => {
  it('a vista completa la imagen general basta (su píxel es más fino que el de la pantalla)', () => {
    // 4 px/mm -> 0,25 mm por píxel de pantalla; la imagen general tiene 0,1 mm.
    expect(planDetail(input())).toEqual({ kind: 'keep' });
  });

  it('al acercar pide solo lo visible, un poco ensanchado, con un píxel de imagen por píxel de pantalla', () => {
    const view = { zoom: 40, panX: -1000, panY: 3000 }; // se ven 10 x 10 mm desde (25, 65)
    const plan = planDetail(input({ view }));
    expect(plan.kind).toBe('request');
    if (plan.kind !== 'request') return;
    expect(plan.region[0]).toBeCloseTo(23, 6);
    expect(plan.region[2]).toBeCloseTo(37, 6);
    // 14 mm / (1/40 mm por píxel) = 560 píxeles.
    expect(plan.maxPx).toBe(560);
  });

  it('con el detalle pedido y vigente no pide otro; al alejarse lo suelta', () => {
    const view = { zoom: 40, panX: -1000, panY: 3000 };
    const first = planDetail(input({ view }));
    if (first.kind !== 'request') throw new Error('debía pedir');
    const detail = { covered: first.region, pxMm: 1 / 40 };
    expect(planDetail(input({ view, detail }))).toEqual({ kind: 'keep' });
    // Mover la vista fuera de lo cubierto pide de nuevo.
    expect(planDetail(input({ view: { ...view, panX: -3000 }, detail }))).toMatchObject({ kind: 'request' });
    // Volver a la vista completa suelta el detalle.
    expect(planDetail(input({ detail }))).toEqual({ kind: 'drop' });
  });

  it('nunca pide más del máximo y no pide nada fuera de la hoja', () => {
    const plan = planDetail(input({ view: { zoom: 60, panX: 0, panY: 6000 }, width: 6000, height: 6000, fullPxMm: 0.5 }));
    expect(plan.kind === 'request' && plan.maxPx).toBe(DETAIL_MAX_PX);
    expect(planDetail(input({ view: { zoom: 4, panX: 5000, panY: 400 }, fullPxMm: 0.5 })).kind).toBe('keep');
  });
});
