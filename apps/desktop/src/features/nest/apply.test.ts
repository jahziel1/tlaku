import { describe, expect, it } from 'vitest';
import { selectionUnits } from '@/features/arrange/units';
import { applyNest } from './apply';
import { sheetOffsets, sheetReferences } from './sheets';
import { GUIDE_COLOR } from '@/lib/palette';
import { docOf, rectObject } from './testUtils';
import type { NestPlacement } from './types';
import { createAppStore } from '@/store';

const place = (piece: string, copy: number, transform: NestPlacement['transform']): NestPlacement => ({ piece, sheet: 0, copy, rotation_deg: 0, mirror: false, transform });

describe('applyNest', () => {
  const a = rectObject('a', 10, 5, [3, 4]);
  const b = rectObject('b', 4, 4, [20, 0]);

  it('copy 0 compone el transform del acomodo por encima del del objeto', () => {
    const doc = docOf([a, b]);
    const units = selectionUnits(doc, ['a', 'b']);
    // giro de 90° antihorario más un desplazamiento
    const r = applyNest(doc, units, [place('a', 0, [0, 1, -1, 0, 100, 200])]);
    expect(r.doc.objects.find((o) => o.id === 'a')!.transform).toEqual([0, 1, -1, 0, 100 + -1 * 4 + 0 * 3, 200 + 3]);
    // la que no se colocó queda idéntica (misma referencia)
    expect(r.doc.objects.find((o) => o.id === 'b')).toBe(b);
    expect(r.placed).toEqual(['a']);
  });

  it('las copias son objetos nuevos con ids únicos, desbloqueados y con el mismo transform compuesto', () => {
    const locked = rectObject('a', 10, 5, [3, 4]);
    const doc = docOf([locked, rectObject('a-1', 1, 1, [50, 50])]);
    const units = selectionUnits(doc, ['a']);
    const r = applyNest(doc, units, [place('a', 0, [1, 0, 0, 1, 10, 0]), place('a', 1, [1, 0, 0, 1, 0, 20]), place('a', 2, [1, 0, 0, 1, 0, 40])]);
    const ids = r.doc.objects.map((o) => o.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(ids).toHaveLength(4);
    const copies = r.doc.objects.slice(2);
    expect(copies.map((o) => o.transform)).toEqual([[1, 0, 0, 1, 3, 24], [1, 0, 0, 1, 3, 44]]);
    expect(copies.every((o) => !o.locked && o.kind === locked.kind)).toBe(true);
    expect(r.placed).toEqual(['a', ...copies.map((o) => o.id)]);
  });

  it('las copias de un grupo forman grupos propios y completos', () => {
    const g1 = rectObject('g1', 5, 5, [0, 0], { group: 'grupo-1' });
    const g2 = rectObject('g2', 5, 5, [8, 0], { group: 'grupo-1' });
    const doc = docOf([g1, g2]);
    const units = selectionUnits(doc, ['g1']);
    expect(units).toHaveLength(1);
    const r = applyNest(doc, units, [place('g1', 0, [1, 0, 0, 1, 1, 1]), place('g1', 1, [1, 0, 0, 1, 0, 30]), place('g1', 2, [1, 0, 0, 1, 0, 60])]);
    expect(r.doc.objects).toHaveLength(6);
    const copies = r.doc.objects.slice(2);
    const groups = copies.map((o) => o.group);
    expect(groups.every((g) => g !== null && g !== 'grupo-1')).toBe(true);
    expect(groups[0]).toBe(groups[1]);
    expect(groups[2]).toBe(groups[3]);
    expect(groups[0]).not.toBe(groups[2]);
    // el original sigue en su grupo
    expect(r.doc.objects[0]!.group).toBe('grupo-1');
  });

  it('ignora piezas desconocidas y lugares repetidos; sin nada que hacer devuelve el mismo documento', () => {
    const doc = docOf([a]);
    const units = selectionUnits(doc, ['a']);
    expect(applyNest(doc, units, [place('zzz', 0, [1, 0, 0, 1, 5, 5])]).doc).toBe(doc);
    expect(applyNest(doc, units, []).doc).toBe(doc);
    const r = applyNest(doc, units, [place('a', 1, [1, 0, 0, 1, 0, 5]), place('a', 1, [1, 0, 0, 1, 0, 9])]);
    expect(r.doc.objects).toHaveLength(2);
  });

  it('en un solo applyDocChange: un solo paso de deshacer', () => {
    const store = createAppStore();
    const doc = docOf([a, b]);
    store.getState().replaceDocument(doc, null);
    const units = selectionUnits(store.getState().doc, ['a', 'b']);
    store.getState().applyDocChange('nest.history', (d) =>
      applyNest(d, units, [place('a', 0, [1, 0, 0, 1, 10, 10]), place('a', 1, [1, 0, 0, 1, 40, 0]), place('b', 0, [1, 0, 0, 1, 0, 30])]).doc,
    );
    expect(store.getState().doc.objects).toHaveLength(3);
    expect(store.getState().past).toHaveLength(1);
    expect(store.getState().past[0]!.label).toBe('nest.history');
    store.getState().undo();
    expect(store.getState().doc).toBe(doc);
  });

  describe('varias hojas', () => {
    const sheet = (index: number, source: 'container' | 'remnant' = 'container', remnant_id: string | null = null) => ({
      index,
      source,
      remnant_id,
      bounds: [0, 0, 100, 60] as [number, number, number, number],
      utilization: 50,
      placed: 1,
      area_mm2: 6000,
      placed_area_mm2: 3000,
      leftovers: [],
    });
    const bed = { type: 'rect', width: 100, height: 60 } as const;

    it('cada hoja se traslada a su zona y la segunda queda a la derecha de la primera', () => {
      const doc = docOf([a, b]);
      const units = selectionUnits(doc, ['a', 'b']);
      const sheets = [sheet(0), sheet(1)];
      const offsets = sheetOffsets(sheets);
      const layout = { offsets, references: sheetReferences(sheets, offsets, bed, new Map(), null) };
      const r = applyNest(doc, units, [{ ...place('a', 0, [1, 0, 0, 1, 10, 10]), sheet: 0 }, { ...place('b', 0, [1, 0, 0, 1, 5, 5]), sheet: 1 }], layout);
      expect(r.doc.objects.find((o) => o.id === 'a')!.transform).toEqual([1, 0, 0, 1, 13, 14]);
      // b está en (20, 0): +5 de la colocación y +120 de la hoja 2
      expect(r.doc.objects.find((o) => o.id === 'b')!.transform).toEqual([1, 0, 0, 1, 145, 5]);
      expect(r.placed).toEqual(['a', 'b']);
    });

    it('agrega un contorno por hoja en la capa de guías, que no se corta, y no lo cuenta como acomodado', () => {
      const doc = docOf([a, b]);
      const units = selectionUnits(doc, ['a', 'b']);
      const sheets = [sheet(0), sheet(1)];
      const offsets = sheetOffsets(sheets);
      const layout = { offsets, references: sheetReferences(sheets, offsets, bed, new Map(), null) };
      const r = applyNest(doc, units, [{ ...place('a', 0, [1, 0, 0, 1, 0, 0]), sheet: 0 }, { ...place('b', 0, [1, 0, 0, 1, 0, 0]), sheet: 1 }], layout);
      const refs = r.doc.objects.filter((o) => o.id.startsWith('hoja-'));
      expect(refs.map((o) => o.id)).toEqual(['hoja-1', 'hoja-2']);
      expect(refs.every((o) => o.layer === GUIDE_COLOR && o.kind.type === 'vector')).toBe(true);
      expect(r.doc.layers.find((l) => l.color === GUIDE_COLOR)?.output).toBe(false);
      expect(r.placed).toEqual(['a', 'b']);
    });

    it('una sola hoja de la cama no agrega nada (como antes) y un retazo sí agrega su contorno', () => {
      const doc = docOf([a]);
      const units = selectionUnits(doc, ['a']);
      const one = [sheet(0)];
      const noRef = applyNest(doc, units, [place('a', 0, [1, 0, 0, 1, 1, 1])], { offsets: [[0, 0]], references: sheetReferences(one, [[0, 0]], bed, new Map(), null) });
      expect(noRef.doc.objects).toHaveLength(1);
      expect(noRef.doc.layers).toHaveLength(0);
      const rem = [sheet(0, 'remnant', 'r')];
      const rems = new Map([['r', { paths: [[[0, 0], [100, 0], [100, 60], [0, 60]] as [number, number][]] }]]);
      const withRef = applyNest(doc, units, [place('a', 0, [1, 0, 0, 1, 1, 1])], { offsets: [[0, 0]], references: sheetReferences(rem, [[0, 0]], bed, rems, null) });
      expect(withRef.doc.objects.map((o) => o.id)).toEqual(['a', 'hoja-1']);
    });

    it('si el id de la hoja ya existe usa otro', () => {
      const doc = docOf([a, { ...rectObject('hoja-1', 5, 5), id: 'hoja-1' }]);
      const units = selectionUnits(doc, ['a']);
      const rem = [sheet(0, 'remnant', 'r')];
      const rems = new Map([['r', { paths: [[[0, 0], [10, 0], [10, 10]] as [number, number][]] }]]);
      const r = applyNest(doc, units, [place('a', 0, [1, 0, 0, 1, 1, 1])], { offsets: [[0, 0]], references: sheetReferences(rem, [[0, 0]], bed, rems, null) });
      expect(r.doc.objects.map((o) => o.id)).toEqual(['a', 'hoja-1', 'hoja-1-2']);
    });
  });
});
