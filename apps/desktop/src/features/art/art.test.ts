import { describe, expect, it } from 'vitest';
import type { ImportResult } from '@/api/types';
import { roundedRectBez } from '@/lib/bezpath';
import { objectBounds } from '@/lib/geometry';
import { ART_CATEGORIES, BUILTIN_ART, filterArt, userItem } from './catalog';
import { placeArt } from './place';

describe('biblioteca de arte', () => {
  it('trae entre 30 y 50 dibujos propios, de todas las categorías pedidas', () => {
    expect(BUILTIN_ART.length).toBeGreaterThanOrEqual(30);
    expect(BUILTIN_ART.length).toBeLessThanOrEqual(50);
    for (const c of ART_CATEGORIES) expect(BUILTIN_ART.some((i) => i.category === c), c).toBe(true);
    expect(new Set(BUILTIN_ART.map((i) => i.id)).size).toBe(BUILTIN_ART.length);
    for (const i of BUILTIN_ART) {
      expect(i.svg, i.id).toContain('<svg');
      expect(i.es && i.en, i.id).toBeTruthy();
    }
  });

  it('filtra por categoría y por texto sin acentos, en español o en inglés', () => {
    const all = [...BUILTIN_ART, userItem('mi logo', '<svg/>')];
    expect(filterArt(all, 'estrellas', '').every((i) => i.category === 'estrellas')).toBe(true);
    expect(filterArt(all, 'todo', 'corazon').some((i) => i.category === 'corazones')).toBe(true);
    expect(filterArt(all, 'todo', 'heart').some((i) => i.category === 'corazones')).toBe(true);
    expect(filterArt(all, 'usuario', '').map((i) => i.es)).toEqual(['mi logo']);
    expect(filterArt(all, 'todo', 'zzzz')).toEqual([]);
  });
});

describe('insertar un dibujo', () => {
  const obj = (id: string, x: number) => ({
    id,
    layer: '#000000',
    transform: [1, 0, 0, 1, x, 0] as [number, number, number, number, number, number],
    kind: { type: 'vector' as const, paths: [roundedRectBez(0, 0, 10, 10)] },
    locked: false,
    hidden: false,
    group: null,
  });

  it('centra el dibujo en el punto y agrupa lo que trae varios objetos', () => {
    const r: ImportResult = { objects: [obj('a', 0), obj('b', 20)], new_layers: [], warnings: [] };
    const placed = placeArt(r, [100, 50], [{ ...obj('z', 0), group: 'grupo-1' }]);
    const boxes = placed.objects.map((o) => objectBounds(o)!);
    const minX = Math.min(...boxes.map((b) => b[0][0]));
    const maxX = Math.max(...boxes.map((b) => b[1][0]));
    expect((minX + maxX) / 2).toBeCloseTo(100);
    expect((boxes[0]![0][1] + boxes[0]![1][1]) / 2).toBeCloseTo(50);
    expect(placed.objects.map((o) => o.group)).toEqual(['grupo-2', 'grupo-2']);
  });

  it('un dibujo de un solo objeto no se agrupa', () => {
    const r: ImportResult = { objects: [obj('a', 0)], new_layers: [], warnings: [] };
    expect(placeArt(r, [0, 0], []).objects[0]!.group).toBeNull();
  });
});
