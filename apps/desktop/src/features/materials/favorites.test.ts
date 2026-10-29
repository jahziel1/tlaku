import { beforeEach, describe, expect, it } from 'vitest';
import type { MaterialPreset } from '@/api/types';
import { sampleLayers, sampleObjects, SAMPLE_COLORS } from '@/api/mock/sample';
import { checkPref, MAX_FAVORITES } from '@/store/prefs';
import { makeLayer } from '@/lib/layers';
import { useStore } from '@/store';
import { emptyDocument } from '@/store/document';
import { applyFavoriteToSelection, favoriteApplyReason, favoriteIds, favoritePresets, isFavorite, pickFavoriteTarget, setFavorite } from './favorites';
import { presetFromLayer } from './library';

const s = () => useStore.getState();
const form = { material: 'mdf', thickness: 3, title: 'Corte limpio', notes: '', device_id: null };
const cutPreset = (id = 'fav-cut', over = {}): MaterialPreset => presetFromLayer(makeLayer('#000000', 'x', { speed: 777, power: 66, passes: 1, ...over }), form, id);

beforeEach(() => {
  s().resetPrefs();
  s().replaceDocument({ ...emptyDocument(), objects: sampleObjects(), layers: sampleLayers() }, null);
  s().select([]);
});

describe('favoritos: guardado en las preferencias', () => {
  it('marcar, desmarcar y no repetir', () => {
    expect(favoriteIds()).toEqual([]);
    expect(setFavorite('a', true)).toBe(true);
    expect(setFavorite('a', true)).toBe(true);
    setFavorite('b', true);
    expect(favoriteIds()).toEqual(['a', 'b']);
    expect(isFavorite('a')).toBe(true);
    setFavorite('a', false);
    expect(favoriteIds()).toEqual(['b']);
  });

  it('tiene un tope', () => {
    for (let i = 0; i < MAX_FAVORITES; i++) expect(setFavorite(`m${i}`, true)).toBe(true);
    expect(setFavorite('uno-mas', true)).toBe(false);
    expect(favoriteIds()).toHaveLength(MAX_FAVORITES);
  });

  it('el saneado descarta lo que no son ids y los repetidos', () => {
    expect(checkPref('favoriteMaterials', ['a', 'a', 3, '', null, 'b'])?.value).toEqual(['a', 'b']);
    expect(checkPref('favoriteMaterials', 'no')).toBeNull();
  });

  it('favoritePresets respeta el orden en que se marcaron y salta los que ya no existen', () => {
    const a = cutPreset('a');
    const b = cutPreset('b');
    expect(favoritePresets([a], [b], ['b', 'borrado', 'a']).map((p) => p.id)).toEqual(['b', 'a']);
  });
});

describe('favoritos: aplicar a la selección', () => {
  it('sin selección no se puede y dice por qué', () => {
    expect(favoriteApplyReason(s(), cutPreset())?.key).toBe('materials.fav.applyReason.noSelection');
    expect(applyFavoriteToSelection(cutPreset())).toBe(false);
  });

  it('crea una capa nueva con los ajustes del material en el primer color libre y mueve la selección', () => {
    s().select(['obj-estrella']);
    const preset = cutPreset();
    const target = pickFavoriteTarget(s().doc, preset)!;
    expect(target.existing).toBe(false);
    expect(s().doc.layers.some((l) => l.color === target.color)).toBe(false);

    expect(applyFavoriteToSelection(preset)).toBe(true);
    const layer = s().doc.layers.find((l) => l.color === target.color)!;
    expect(layer).toMatchObject({ speed: 777, power: 66, output: true });
    expect(s().doc.objects.find((o) => o.id === 'obj-estrella')!.layer).toBe(target.color);
    // El resto del diseño no se tocó.
    expect(s().doc.objects.find((o) => o.id === 'obj-placa')!.layer).toBe(SAMPLE_COLORS.cut);
    expect(s().activeLayer).toBe(target.color);
  });

  it('si ya hay una capa con esos ajustes, la reutiliza en vez de crear otra', () => {
    s().select(['obj-estrella']);
    const preset = presetFromLayer(s().doc.layers.find((l) => l.color === SAMPLE_COLORS.cut)!, form, 'fav-same');
    const before = s().doc.layers.length;
    expect(pickFavoriteTarget(s().doc, preset)).toEqual({ color: SAMPLE_COLORS.cut, existing: true });
    expect(applyFavoriteToSelection(preset)).toBe(true);
    expect(s().doc.layers).toHaveLength(before);
    expect(s().doc.objects.find((o) => o.id === 'obj-estrella')!.layer).toBe(SAMPLE_COLORS.cut);
  });

  it('es un solo paso de deshacer', () => {
    s().select(['obj-estrella']);
    const before = s().doc;
    applyFavoriteToSelection(cutPreset());
    expect(s().doc).not.toBe(before);
    s().undo();
    expect(s().doc.objects.find((o) => o.id === 'obj-estrella')!.layer).toBe(before.objects.find((o) => o.id === 'obj-estrella')!.layer);
    expect(s().doc.layers).toHaveLength(before.layers.length);
  });

  it('un material de línea o relleno no se aplica a una imagen, y al revés', () => {
    s().select(['obj-foto']);
    expect(favoriteApplyReason(s(), cutPreset())?.key).toBe('materials.fav.applyReason.mode');
    s().select(['obj-estrella']);
    expect(favoriteApplyReason(s(), cutPreset('img', { mode: 'image' }))?.key).toBe('materials.fav.applyReason.mode');
    s().select(['obj-foto']);
    expect(favoriteApplyReason(s(), cutPreset('img', { mode: 'image' }))).toBeNull();
  });

  it('los objetos bloqueados no se mueven', () => {
    s().select(['obj-estrella']);
    s().applyDocChange('history.editObjects', (d) => ({ ...d, objects: d.objects.map((o) => (o.id === 'obj-estrella' ? { ...o, locked: true } : o)) }));
    expect(favoriteApplyReason(s(), cutPreset())?.key).toBe('materials.fav.applyReason.locked');
  });
});
