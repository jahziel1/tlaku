import { beforeEach, describe, expect, it } from 'vitest';
import type { DesignObject } from '@/api/types';
import { lockedInLayer } from '@/features/layers/actions';
import { polylineBez } from '@/lib/bezpath';
import { countByLayer, makeLayer } from '@/lib/layers';
import { createAppStore } from './index';

function obj(id: string, layer = '#ff0000', patch: Partial<DesignObject> = {}): DesignObject {
  return {
    id, layer, transform: [1, 0, 0, 1, 0, 0], locked: false, hidden: false, group: null,
    kind: { type: 'vector', paths: [polylineBez([[0, 0], [1, 0], [1, 1]], true)] }, ...patch,
  };
}

let store: ReturnType<typeof createAppStore>;
const s = () => store.getState();

beforeEach(() => {
  store = createAppStore();
  s().applyImport({
    objects: [obj('a'), obj('b'), obj('c', '#0000ff'), obj('d', '#ff0000', { locked: true })],
    new_layers: [makeLayer('#ff0000', 'Rojo'), makeLayer('#0000ff', 'Azul')],
    warnings: [],
  });
});

describe('pruneSelection por índice de ids', () => {
  it('quita lo borrado, oculto, bloqueado o de una capa invisible, y conserva el orden y la identidad si nada cambia', () => {
    s().select(['a', 'b', 'c']);
    const before = s().selectedIds;
    s().pruneSelection();
    expect(s().selectedIds).toBe(before);
    s().applyDocChange('history.editObjects', (d) => ({ ...d, objects: d.objects.map((o) => (o.id === 'b' ? { ...o, hidden: true } : o)) }));
    s().pruneSelection();
    expect(s().selectedIds).toEqual(['a', 'c']);
    s().applyDocChange('history.editObjects', (d) => ({ ...d, layers: d.layers.map((l) => (l.color === '#0000ff' ? { ...l, visible: false } : l)) }));
    s().pruneSelection();
    expect(s().selectedIds).toEqual(['a']);
    s().deleteObjects(['a']);
    s().pruneSelection();
    expect(s().selectedIds).toEqual([]);
  });

  it('deshacer y rehacer podan la selección', () => {
    s().moveObjects(['a'], 1, 0, 'k');
    s().select(['a']);
    s().deleteObjects(['b']);
    s().select(['a', 'b'].filter((id) => id === 'a'));
    s().undo();
    s().undo();
    expect(s().selectedIds).toEqual(['a']);
    // 'x' no existe en el documento: se descarta
    s().select(['a', 'x']);
    s().pruneSelection();
    expect(s().selectedIds).toEqual(['a']);
  });
});

describe('conteos por capa guardados por documento', () => {
  it('countByLayer y lockedInLayer dan lo de siempre y no recalculan con la misma lista de objetos', () => {
    const doc = s().doc;
    const counts = countByLayer(doc);
    expect(counts.get('#ff0000')).toEqual({ total: 3, live: 3 });
    expect(counts.get('#0000ff')).toEqual({ total: 1, live: 1 });
    expect(countByLayer(doc)).toBe(counts);
    expect(countByLayer({ ...doc, layers: [...doc.layers] })).not.toBe(counts); // otras capas: se recalcula
    expect(lockedInLayer(doc, '#ff0000')).toBe(1);
    expect(lockedInLayer(doc, '#FF0000')).toBe(1);
    expect(lockedInLayer(doc, '#0000ff')).toBe(0);
    s().moveObjects(['a'], 1, 0, 'k');
    expect(countByLayer(s().doc)).not.toBe(counts);
  });
});
