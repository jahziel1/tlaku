import { describe, expect, it } from 'vitest';
import type { DesignObject, Document } from '@/api/types';
import { createAppStore } from '@/store';
import { ALL_COMMANDS } from '@/commands';
import { CommandRegistry } from '@/commands/registry';
import { newVectorObject } from '@/features/canvas/shapes';
import { polylineBez } from './bezpath';
import { expandGroups, groupDoc, groupIdsOf, groupMembers, regroupCopies, ungroupDoc } from './groups';

const o = (id: string, patch: Partial<DesignObject> = {}): DesignObject => ({
  ...newVectorObject(id, '#ff0000', [polylineBez([[0, 0], [1, 1]], false)]),
  ...patch,
});
const docOf = (objects: DesignObject[]): Document => ({ version: 2, objects, layers: [], device_id: null });

describe('grupos', () => {
  it('agrupa a dos o más objetos con un id nuevo y no toca a los demás', () => {
    const d = docOf([o('a'), o('b'), o('c')]);
    const g = groupDoc(d, ['a', 'c']);
    expect(g.objects[0].group).toBe('grupo-1');
    expect(g.objects[2].group).toBe('grupo-1');
    expect(g.objects[1].group).toBeNull();
    expect(g.objects[1]).toBe(d.objects[1]); // sin cambios: misma referencia
    expect(groupMembers(g.objects, 'grupo-1')).toEqual(['a', 'c']);
    expect(groupDoc(d, ['a'])).toBe(d); // uno solo no es un grupo
    expect(groupDoc(d, ['a', 'zzz'])).toBe(d);
  });

  it('los bloqueados no entran al grupo', () => {
    const d = docOf([o('a'), o('b', { locked: true }), o('c')]);
    const g = groupDoc(d, ['a', 'b', 'c']);
    expect(g.objects.map((x) => x.group)).toEqual(['grupo-1', null, 'grupo-1']);
  });

  it('agrupar de nuevo mueve los objetos al grupo nuevo y disuelve el que queda con uno', () => {
    const g1 = groupDoc(docOf([o('a'), o('b'), o('c'), o('d')]), ['a', 'b', 'c']);
    const g2 = groupDoc(g1, ['c', 'd']); // c deja grupo-1 (quedan a y b) y se une a d
    expect(g2.objects.map((x) => x.group)).toEqual(['grupo-1', 'grupo-1', 'grupo-2', 'grupo-2']);
    const g3 = groupDoc(g1, ['b', 'd']); // b sale de grupo-1 (quedan a y c)
    expect(g3.objects.map((x) => x.group)).toEqual(['grupo-1', 'grupo-2', 'grupo-1', 'grupo-2']);
    const g4 = groupDoc(docOf([o('a', { group: 'x' }), o('b', { group: 'x' }), o('c')]), ['b', 'c']);
    expect(g4.objects.map((x) => x.group)).toEqual([null, 'grupo-1', 'grupo-1']); // 'x' se quedó con uno solo
  });

  it('desagrupar disuelve el grupo entero', () => {
    const g = groupDoc(docOf([o('a'), o('b'), o('c')]), ['a', 'b', 'c']);
    const u = ungroupDoc(g, ['a']);
    expect(u.objects.every((x) => x.group === null)).toBe(true);
    expect(ungroupDoc(u, ['a'])).toBe(u);
  });

  it('expandGroups suma los miembros del grupo (solo los que cumplan keep)', () => {
    const d = docOf([o('a', { group: 'g' }), o('b', { group: 'g' }), o('c', { group: 'g', locked: true }), o('d')]);
    expect(expandGroups(d.objects, ['a'])).toEqual(['a', 'b', 'c']);
    expect(expandGroups(d.objects, ['a'], (x) => !x.locked)).toEqual(['a', 'b']);
    expect(expandGroups(d.objects, ['d'])).toEqual(['d']);
    expect(expandGroups(d.objects, ['a', 'b'])).toEqual(['a', 'b', 'c']);
  });

  it('las copias reciben grupos propios', () => {
    const copies = [o('a2', { group: 'g' }), o('b2', { group: 'g' }), o('c2', { group: 'h' }), o('d2')];
    const out = regroupCopies(copies, new Set(['g', 'h']));
    expect(out[0].group).toBe(out[1].group);
    expect(out[0].group).not.toBe('g');
    expect(out[2].group).toBeNull(); // su grupo no se copió completo
    expect(groupIdsOf(out).size).toBe(1);
  });
});

describe('comandos agrupar / desagrupar', () => {
  const setup = () => {
    const store = createAppStore();
    store.getState().replaceDocument(docOf([o('a'), o('b'), o('c')]), null);
    return { store, registry: new CommandRegistry(ALL_COMMANDS, () => store.getState()) };
  };

  it('Agrupar necesita dos objetos, es un paso de historial y se deshace', async () => {
    const { store, registry } = setup();
    store.getState().select(['a']);
    expect(registry.availability('edit.group')).toBe('history.reason.groupNeedsTwo');
    store.getState().select(['a', 'b']);
    expect(registry.availability('edit.group')).toBe(true);
    await registry.run('edit.group');
    const s = store.getState();
    expect(s.doc.objects.map((x) => x.group)).toEqual(['grupo-1', 'grupo-1', null]);
    expect(s.past).toHaveLength(1);
    s.undo();
    expect(store.getState().doc.objects.every((x) => x.group === null)).toBe(true);
    store.getState().redo();
    expect(store.getState().doc.objects[0].group).toBe('grupo-1');
  });

  it('Desagrupar solo se ofrece si hay grupo', async () => {
    const { store, registry } = setup();
    store.getState().select(['a', 'b']);
    expect(registry.availability('edit.ungroup')).toBe('history.reason.noGroup');
    await registry.run('edit.group');
    expect(registry.availability('edit.ungroup')).toBe(true);
    await registry.run('edit.ungroup');
    expect(store.getState().doc.objects.every((x) => x.group === null)).toBe(true);
  });

  it('duplicar y pegar un grupo crea otro grupo, no se une al original', async () => {
    const { store, registry } = setup();
    store.getState().select(['a', 'b']);
    await registry.run('edit.group');
    await registry.run('edit.duplicate');
    let groups = groupIdsOf(store.getState().doc.objects);
    expect(groups.size).toBe(2);
    store.getState().copyObjects(['a', 'b']);
    store.getState().pasteObjects();
    groups = groupIdsOf(store.getState().doc.objects);
    expect(groups.size).toBe(3);
    const original = store.getState().doc.objects.filter((x) => x.group === 'grupo-1');
    expect(original.map((x) => x.id)).toEqual(['a', 'b']);
  });

  it('Ctrl+G y Ctrl+Mayús+G están asignados', () => {
    const { registry } = setup();
    expect(registry.match('Mod+G')?.id).toBe('edit.group');
    expect(registry.match('Mod+Shift+G')?.id).toBe('edit.ungroup');
  });
});
