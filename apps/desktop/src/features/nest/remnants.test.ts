import { describe, expect, it } from 'vitest';
import {
  autoName,
  freshRemnantId,
  matchingRemnants,
  MAX_REMNANT_NAME,
  netArea,
  normalizeMaterial,
  remnantFromLeftover,
  remnantInputs,
  sameStock,
  type Remnant,
} from './remnants';

const rect = (x0: number, y0: number, x1: number, y1: number): [number, number][] => [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
const form = { material: ' Triplay ', thicknessMm: 3, id: 'retazo-1', now: 1700 };

function remnant(over: Partial<Remnant> = {}): Remnant {
  return {
    id: 'r',
    name: 'Retazo',
    material: 'Triplay',
    thickness_mm: 3,
    width_mm: 100,
    height_mm: 50,
    area_mm2: 5000,
    rect: true,
    paths: [rect(0, 0, 100, 50)],
    created_ms: 1,
    ...over,
  };
}

describe('remnantFromLeftover', () => {
  it('lleva el contorno al origen y mide su caja y su área', () => {
    const r = remnantFromLeftover([rect(100, 40, 300, 140)], form)!;
    expect(r.paths).toEqual([rect(0, 0, 200, 100)]);
    expect([r.width_mm, r.height_mm, r.area_mm2, r.rect]).toEqual([200, 100, 20000, true]);
    expect(r).toMatchObject({ id: 'retazo-1', material: 'Triplay', thickness_mm: 3, created_ms: 1700 });
    expect(r.name).toBe('Triplay 3 mm · 200×100');
  });

  it('el área descuenta los huecos y una forma con hueco no es rectángulo', () => {
    const r = remnantFromLeftover([rect(0, 0, 100, 100), rect(40, 40, 60, 60)], form)!;
    expect(r.area_mm2).toBe(10000 - 400);
    expect(r.rect).toBe(false);
    expect(r.paths).toHaveLength(2);
  });

  it('una forma en L no es rectángulo y conserva su área real', () => {
    const l: [number, number][] = [[0, 0], [100, 0], [100, 40], [40, 40], [40, 100], [0, 100]];
    const r = remnantFromLeftover([l], form)!;
    expect(r.rect).toBe(false);
    expect(r.area_mm2).toBe(100 * 40 + 40 * 60);
  });

  it('usa el nombre dado y lo recorta; sin contorno válido da null', () => {
    expect(remnantFromLeftover([rect(0, 0, 10, 10)], { ...form, name: '  Mi retazo ' })!.name).toBe('Mi retazo');
    expect(remnantFromLeftover([rect(0, 0, 10, 10)], { ...form, name: 'x'.repeat(200) })!.name).toHaveLength(MAX_REMNANT_NAME);
    expect(remnantFromLeftover([], form)).toBeNull();
    expect(remnantFromLeftover([[[0, 0], [1, 1]]], form)).toBeNull();
    expect(remnantFromLeftover([[[0, 0], [Number.NaN, 1], [2, 2]]], form)).toBeNull();
    expect(remnantFromLeftover([[[0, 0], [5, 0], [10, 0]]], form)).toBeNull();
  });

  it('un grosor inválido queda en 0', () => {
    expect(remnantFromLeftover([rect(0, 0, 10, 10)], { ...form, thicknessMm: Number.NaN })!.thickness_mm).toBe(0);
    expect(remnantFromLeftover([rect(0, 0, 10, 10)], { ...form, thicknessMm: -2 })!.thickness_mm).toBe(0);
  });
});

describe('material y grosor', () => {
  it('el material se compara sin mayúsculas, acentos ni espacios de más', () => {
    expect(normalizeMaterial('  Acrílico   Negro ')).toBe('acrilico negro');
    expect(sameStock(remnant({ material: 'ACRILICO' }), 'acrílico', 3)).toBe(true);
    expect(sameStock(remnant(), 'MDF', 3)).toBe(false);
    expect(sameStock(remnant(), '', 3)).toBe(false);
  });

  it('el grosor coincide con una tolerancia; si alguno no lo indica solo cuenta el material', () => {
    expect(sameStock(remnant({ thickness_mm: 3 }), 'Triplay', 3.04)).toBe(true);
    expect(sameStock(remnant({ thickness_mm: 3 }), 'Triplay', 6)).toBe(false);
    expect(sameStock(remnant({ thickness_mm: 0 }), 'Triplay', 6)).toBe(true);
    expect(sameStock(remnant({ thickness_mm: 3 }), 'Triplay', 0)).toBe(true);
  });

  it('los retazos que sirven van de menor a mayor área', () => {
    const list = [
      remnant({ id: 'a', area_mm2: 900 }),
      remnant({ id: 'b', area_mm2: 100 }),
      remnant({ id: 'c', material: 'MDF', area_mm2: 1 }),
      remnant({ id: 'd', thickness_mm: 6, area_mm2: 2 }),
      remnant({ id: 'e', area_mm2: 400 }),
    ];
    expect(matchingRemnants(list, 'triplay', 3).map((r) => r.id)).toEqual(['b', 'e', 'a']);
    expect(remnantInputs([list[0]!])).toEqual([{ id: 'a', paths: list[0]!.paths }]);
  });
});

describe('utilidades', () => {
  it('área neta: el primer contorno es el exterior y los demás, huecos', () => {
    expect(netArea([rect(0, 0, 10, 10), rect(2, 2, 4, 4)])).toBe(96);
    expect(netArea([])).toBe(0);
  });

  it('nombre automático sin material ni grosor', () => {
    expect(autoName('', 0, 99.6, 10)).toBe('Retazo · 100×10');
    expect(autoName('MDF', 2.5, 50, 40)).toBe('MDF 2.5 mm · 50×40');
  });

  it('el id nuevo no choca con los existentes', () => {
    const first = freshRemnantId(new Set(), 1700);
    expect(freshRemnantId(new Set([first]), 1700)).not.toBe(first);
  });
});
