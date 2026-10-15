import { describe, expect, it } from 'vitest';
import { flatPolylines } from './preview';
import { offsetPlacements, referenceSheets, SHEET_GAP_MM, sheetOffsets, sheetOutline, sheetReferences, sheetsBounds } from './sheets';
import { rectObject } from './testUtils';
import type { NestPlacement, NestSheet } from './types';

const sheet = (index: number, bounds: NestSheet['bounds'], over: Partial<NestSheet> = {}): NestSheet => ({
  index,
  source: 'container',
  remnant_id: null,
  bounds,
  utilization: 50,
  placed: 1,
  area_mm2: 1,
  placed_area_mm2: 1,
  leftovers: [],
  ...over,
});

const place = (sheetIndex: number, transform: NestPlacement['transform']): NestPlacement => ({ piece: 'a', sheet: sheetIndex, copy: 0, rotation_deg: 0, mirror: false, transform });

describe('sheetOffsets', () => {
  it('la primera hoja no se mueve y las demás van en fila a la derecha, alineadas por abajo', () => {
    const sheets = [sheet(0, [0, 0, 100, 60]), sheet(1, [0, 0, 100, 60]), sheet(2, [10, 5, 50, 45])];
    const off = sheetOffsets(sheets);
    expect(off[0]).toEqual([0, 0]);
    expect(off[1]).toEqual([100 + SHEET_GAP_MM, 0]);
    // la tercera mide 40 de ancho y su esquina inferior izquierda es (10, 5): termina en x = 240 + 40
    expect(off[2]).toEqual([220 + SHEET_GAP_MM - 10, -5]);
    expect(sheetsBounds(sheets, off)).toEqual([[0, 0], [280, 60]]);
  });

  it('un retazo normalizado en el origen tras la primera hoja queda a la separación pedida', () => {
    const off = sheetOffsets([sheet(0, [0, 0, 100, 60]), sheet(1, [0, 0, 30, 20], { source: 'remnant', remnant_id: 'r' })], 5);
    expect(off[1]).toEqual([105, 0]);
  });

  it('sin hojas no hay traslados ni caja', () => {
    expect(sheetOffsets([])).toEqual([]);
    expect(sheetsBounds([], [])).toBeNull();
  });
});

describe('offsetPlacements', () => {
  it('suma el traslado de la hoja y deja intactas las de la primera', () => {
    const off: [number, number][] = [[0, 0], [120, 7]];
    const a = place(0, [1, 0, 0, 1, 3, 4]);
    const b = place(1, [0, 1, -1, 0, 3, 4]);
    const out = offsetPlacements([a, b], off);
    expect(out[0]).toBe(a);
    expect(out[1]!.transform).toEqual([0, 1, -1, 0, 123, 11]);
  });

  it('una hoja desconocida no se mueve', () => {
    const a = place(5, [1, 0, 0, 1, 3, 4]);
    expect(offsetPlacements([a], [[0, 0]])[0]).toBe(a);
  });
});

describe('contornos de referencia', () => {
  const remnants = new Map([['r', { paths: [[[0, 0], [30, 0], [30, 10], [0, 10]] as [number, number][]] }]]);
  const material = flatPolylines([rectObject('m', 50, 40, [10, 10])]);

  it('con una sola hoja de la cama no se dibuja nada; con un retazo sí', () => {
    expect(referenceSheets([sheet(0, [0, 0, 100, 60])], { type: 'rect', width: 100, height: 60 })).toEqual([]);
    expect(referenceSheets([sheet(0, [0, 0, 30, 10], { source: 'remnant', remnant_id: 'r' })], { type: 'rect', width: 100, height: 60 })).toEqual([0]);
  });

  it('con varias hojas todas llevan contorno, salvo la primera si ya es un objeto del documento', () => {
    const two = [sheet(0, [0, 0, 100, 60]), sheet(1, [0, 0, 100, 60])];
    expect(referenceSheets(two, { type: 'rect', width: 100, height: 60 })).toEqual([0, 1]);
    expect(referenceSheets(two, { type: 'object', object: rectObject('m', 50, 40) })).toEqual([1]);
  });

  it('el contorno es el rectángulo de la hoja, el del retazo o el del material, ya trasladado', () => {
    const bed = { type: 'rect', width: 100, height: 60 } as const;
    expect(sheetOutline(sheet(0, [0, 0, 100, 60]), bed, remnants, null)[0]!.points).toEqual([[0, 0], [100, 0], [100, 60], [0, 60]]);
    const rem = sheet(1, [0, 0, 30, 10], { source: 'remnant', remnant_id: 'r' });
    expect(sheetOutline(rem, bed, remnants, null)[0]!.points).toHaveLength(4);
    const obj = { type: 'object', object: rectObject('m', 50, 40, [10, 10]) } as const;
    expect(sheetOutline(sheet(0, [10, 10, 60, 50]), obj, remnants, material)[0]!.points[0]).toEqual([10, 10]);

    const sheets = [sheet(0, [0, 0, 100, 60]), rem];
    const refs = sheetReferences(sheets, sheetOffsets(sheets), bed, remnants, null);
    expect(refs.map((r) => r.sheet)).toEqual([0, 1]);
    expect(refs[1]!.polys[0]!.points[0]).toEqual([120, 0]);
    expect(refs[1]!.polys[0]!.closed).toBe(true);
  });

  it('un retazo que ya no está en el inventario se dibuja con la caja de la hoja', () => {
    const rem = sheet(0, [0, 0, 30, 10], { source: 'remnant', remnant_id: 'borrado' });
    expect(sheetOutline(rem, null, new Map(), null)[0]!.points).toEqual([[0, 0], [30, 0], [30, 10], [0, 10]]);
  });
});
