import { describe, expect, it } from 'vitest';
import type { DesignObject } from '@/api/types';
import { polylineBez } from '@/lib/bezpath';
import { lineObj, rectObj } from '@/features/canvas/testUtil';
import { autoGroupObjects, containmentGroups } from './autogroup';

const ring = (id: string, x0: number, y0: number, x1: number, y1: number): DesignObject => rectObj(id, x0, y0, x1, y1);

describe('agrupar por contención', () => {
  it('una pieza con su hueco y su grabado forma un grupo', () => {
    const objs = [ring('pieza', 0, 0, 100, 100), ring('hueco', 10, 10, 30, 30), ring('grabado', 50, 50, 80, 80), ring('suelta', 200, 0, 220, 20)];
    expect(containmentGroups(objs)).toEqual([['pieza', 'hueco', 'grabado']]);
  });

  it('dos piezas con sus huecos forman dos grupos', () => {
    const objs = [ring('a', 0, 0, 50, 50), ring('a-h', 10, 10, 20, 20), ring('b', 100, 0, 150, 50), ring('b-h', 110, 10, 120, 20)];
    expect(containmentGroups(objs)).toEqual([
      ['a', 'a-h'],
      ['b', 'b-h'],
    ]);
  });

  it('una isla dentro de un hueco es una pieza nueva, no parte de la de afuera', () => {
    const objs = [ring('marco', 0, 0, 100, 100), ring('hueco', 10, 10, 90, 90), ring('isla', 30, 30, 60, 60), ring('isla-h', 40, 40, 50, 50)];
    expect(containmentGroups(objs)).toEqual([
      ['marco', 'hueco'],
      ['isla', 'isla-h'],
    ]);
  });

  it('lo que ya está agrupado, las líneas abiertas y los bloqueados no cuentan', () => {
    const objs = [
      ring('pieza', 0, 0, 100, 100),
      { ...ring('en-grupo', 10, 10, 20, 20), group: 'g1' },
      lineObj('linea', [30, 30], [40, 40]),
      { ...ring('bloqueado', 50, 50, 60, 60), locked: true },
    ];
    expect(containmentGroups(objs)).toEqual([]);
  });

  it('formas que se solapan sin contenerse no se agrupan', () => {
    expect(containmentGroups([ring('a', 0, 0, 50, 50), ring('b', 30, 30, 80, 80)])).toEqual([]);
  });

  it('los objetos con curvas (círculos) también cuentan', () => {
    const circle = (id: string, r: number): DesignObject => ({
      ...rectObj(id, 0, 0, 1, 1),
      kind: { type: 'vector', paths: [polylineBez(Array.from({ length: 40 }, (_, i) => [50 + r * Math.cos((i / 40) * 2 * Math.PI), 50 + r * Math.sin((i / 40) * 2 * Math.PI)] as [number, number]), true)] },
    });
    expect(containmentGroups([circle('grande', 40), circle('chico', 10)])).toEqual([['grande', 'chico']]);
  });

  it('autoGroupObjects asigna ids de grupo nuevos que no chocan con los existentes', () => {
    const objs = [ring('pieza', 0, 0, 100, 100), ring('hueco', 10, 10, 30, 30), ring('suelta', 200, 0, 220, 20)];
    const r = autoGroupObjects(objs, new Set(['grupo-1']));
    expect(r.groups).toBe(1);
    expect(r.objects.map((o) => o.group)).toEqual(['grupo-2', 'grupo-2', null]);
    // Sin nada que agrupar devuelve la misma lista.
    const alone = [ring('a', 0, 0, 10, 10)];
    expect(autoGroupObjects(alone).objects).toBe(alone);
  });
});
