import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it } from 'vitest';
import { CommandRegistry } from '@/commands/registry';
import { ALL_COMMANDS } from '@/commands';
import { useStore } from '@/store';
import { commonValue, normalizePowerScale, normalizePriority, setCutProps, stepPriority } from './cutProps';
import { priorityBadges } from './PriorityBadges';
import { ShapePropsView } from './ShapeProps';
import { rectObj } from './testUtil';

const st = () => useStore.getState();
const obj = (id: string) => st().doc.objects.find((o) => o.id === id)!;

beforeEach(() => {
  st().replaceDocument({ version: 2, objects: [], layers: [], device_id: null }, null);
  st().addObjects([rectObj('a', 0, 0, 5, 5), rectObj('b', 10, 0, 15, 5, { priority: 3 }), rectObj('c', 20, 0, 25, 5, { group: 'g2/g1' })]);
});

describe('prioridad y escala de potencia por objeto', () => {
  it('se normalizan: prioridad entera, potencia de 0 a 100 y 100 = sin escala', () => {
    expect(normalizePriority(2.6)).toBe(3);
    expect(normalizePriority(null)).toBeNull();
    expect(normalizePriority(1e9)).toBe(9999);
    expect(normalizePowerScale(250)).toBeNull();
    expect(normalizePowerScale(100)).toBeNull();
    expect(normalizePowerScale(-5)).toBe(0);
    expect(normalizePowerScale(40)).toBe(40);
  });

  it('se aplican a toda la selección en un solo paso y se deshacen', () => {
    setCutProps(st(), ['a', 'b', 'c'], { priority: 2 });
    setCutProps(st(), ['a', 'b'], { power_scale: 60 });
    expect(['a', 'b', 'c'].map((id) => obj(id).priority)).toEqual([2, 2, 2]);
    expect(['a', 'b', 'c'].map((id) => obj(id).power_scale ?? null)).toEqual([60, 60, null]);
    st().undo();
    expect(['a', 'b'].map((id) => obj(id).power_scale ?? null)).toEqual([null, null]);
    expect(obj('a').priority).toBe(2);
    st().undo();
    expect([obj('a').priority ?? null, obj('b').priority]).toEqual([null, 3]);
  });

  it('null devuelve a automática y no deja campos vacíos en el objeto', () => {
    setCutProps(st(), ['b'], { priority: null });
    expect('priority' in obj('b')).toBe(false);
    setCutProps(st(), ['b'], { power_scale: 100 });
    expect('power_scale' in obj('b')).toBe(false);
  });

  it('con varios objetos distintos el valor común es "mixed"', () => {
    const objs = st().doc.objects;
    expect(commonValue(objs, (o) => o.priority)).toBe('mixed');
    expect(commonValue([objs[0]!], (o) => o.priority)).toBeNull();
    expect(commonValue([objs[1]!], (o) => o.priority)).toBe(3);
  });

  it('subir y bajar suma a cada uno (automática cuenta como 0) y respeta grupos y deshacer', () => {
    stepPriority(st(), ['a', 'b', 'c'], -1);
    expect(['a', 'b', 'c'].map((id) => obj(id).priority)).toEqual([-1, 2, -1]);
    expect(obj('c').group).toBe('g2/g1');
    st().undo();
    expect(obj('b').priority).toBe(3);
  });

  it('los comandos de prioridad operan sobre la selección', () => {
    const registry = new CommandRegistry(ALL_COMMANDS, () => st());
    expect(registry.match('Alt+Shift+]')?.id).toBe('arrange.priority.later');
    st().select(['a', 'b']);
    registry.run('arrange.priority.later');
    expect([obj('a').priority, obj('b').priority]).toEqual([1, 4]);
  });

  it('los números sobre el lienzo solo salen para objetos con prioridad dentro de la vista', () => {
    const view = { zoom: 2, panX: 0, panY: 100 };
    const badges = priorityBadges(st().doc.objects, view, { width: 800, height: 600 });
    expect(badges.map((b) => [b.id, b.n])).toEqual([['b', 3]]);
    expect(priorityBadges(st().doc.objects, { zoom: 2, panX: -5000, panY: 100 }, { width: 800, height: 600 })).toEqual([]);
  });

  it('la ventana muestra los campos de corte, la ayuda y "Varios" con selección mixta', () => {
    const noop = () => {};
    const html = renderToString(
      <ShapePropsView params={null} mixed={false} cut={{ priority: 'mixed', power: 40 }} onApply={noop} onCut={noop} onClose={noop} onLock={noop} />,
    );
    for (const text of ['Propiedades del objeto', 'Prioridad de corte', 'Escala de potencia', 'Varios', 'value="40"', 'solo ordena dentro de la capa']) expect(html, text).toContain(text);
  });
});
