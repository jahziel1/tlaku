import { describe, expect, it } from 'vitest';
import type { DesignObject, Document } from '@/api/types';
import { followAnchors, invertAffine } from './anchors';
import { applyAffine } from './geometry';

const obj = (extra: Partial<DesignObject> = {}): DesignObject => ({
  id: 'a', layer: '#ff0000', transform: [1, 0, 0, 1, 0, 0], kind: { type: 'vector', paths: [] }, locked: false, hidden: false, group: null, ...extra,
});
const doc = (...objects: DesignObject[]): Document => ({ version: 2, objects, layers: [], device_id: null });

describe('afín inversa', () => {
  it('deshace la transformación', () => {
    const m: [number, number, number, number, number, number] = [0, 2, -2, 0, 5, -3];
    const inv = invertAffine(m)!;
    const p = applyAffine(inv, applyAffine(m, [3, 4]));
    expect(p[0]).toBeCloseTo(3);
    expect(p[1]).toBeCloseTo(4);
    expect(invertAffine([0, 0, 0, 0, 1, 1])).toBeNull();
  });
});

describe('followAnchors', () => {
  const base = obj({ manual_tabs: [[10, 0], [20, 5]], start: { point: [3, 3], reverse: true } });

  it('al mover el objeto, puentes e inicio lo siguen', () => {
    const prev = doc(base);
    const next = doc({ ...base, transform: [1, 0, 0, 1, 7, -2] });
    const out = followAnchors(prev, next);
    expect(out.objects[0]!.manual_tabs).toEqual([[17, -2], [27, 3]]);
    expect(out.objects[0]!.start).toEqual({ point: [10, 1], reverse: true });
  });

  it('al girar y escalar también', () => {
    const prev = doc(base);
    const next = doc({ ...base, transform: [0, 2, -2, 0, 0, 0] });
    const out = followAnchors(prev, next);
    expect(out.objects[0]!.manual_tabs![0]).toEqual([0, 20]);
  });

  it('si el cambio ya tocó los puentes, los respeta', () => {
    const prev = doc(base);
    const next = doc({ ...base, transform: [1, 0, 0, 1, 7, 0], manual_tabs: [[1, 1]] });
    expect(followAnchors(prev, next)).toBe(next);
  });

  it('no toca lo que no cambió de transformación ni lo que no tiene anclas', () => {
    const prev = doc(base, obj({ id: 'b' }));
    const next = doc(base, obj({ id: 'b', transform: [1, 0, 0, 1, 1, 1] }));
    expect(followAnchors(prev, next)).toBe(next);
  });

  it('un inicio sin punto (solo sentido) no necesita ajuste', () => {
    const o = obj({ start: { point: null, reverse: true } });
    const next = doc({ ...o, transform: [1, 0, 0, 1, 4, 4] });
    expect(followAnchors(doc(o), next)).toBe(next);
  });
});
