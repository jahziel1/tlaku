// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setApi } from '@/api';
import { createMockApi } from '@/api/mock';
import { sampleLayers, sampleObjects } from '@/api/mock/sample';
import { usePrompts } from '@/features/history/prompt';
import { setLanguage } from '@/i18n';
import { selectActiveDevice, useStore } from '@/store';
import { emptyDocument } from '@/store/document';
import { cleanup, click, mount } from '@/test/dom';
import { favoriteIds } from './favorites';
import { deleteMaterial, loadMaterials } from './actions';
import { MaterialsPanelView } from './MaterialsPanel';
import { emptyLibrary } from './library';
import { useMaterials } from './state';

const s = () => useStore.getState();

function Panel() {
  const library = useMaterials((m) => m.library);
  const factory = useMaterials((m) => m.factory);
  const device = useStore(selectActiveDevice);
  const layers = useStore((st) => st.doc.layers);
  const test = { enabled: true, tooltip: '', label: 'Prueba', run: () => undefined };
  return <MaterialsPanelView {...{ library, factory, loaded: true, loadError: null, search: '', onlyDevice: false, devices: [device], device, layers, activeLayer: null, test }} />;
}

const stars = () => [...document.querySelectorAll<HTMLButtonElement>('.mat-star')];
const favSection = () => document.querySelector('section[aria-label="Favoritos"]');

beforeEach(async () => {
  s().resetPrefs();
  setLanguage('es');
  setApi(createMockApi({ latencyMs: 0 }));
  useMaterials.setState({ library: emptyLibrary(), factory: emptyLibrary(), loaded: false, loadError: null, search: '' });
  await loadMaterials();
  s().replaceDocument({ ...emptyDocument(), objects: sampleObjects(), layers: sampleLayers() }, null);
  s().select([]);
});
afterEach(async () => {
  await cleanup();
  s().resetPrefs();
});

describe('estrella de favoritos en la pestaña Materiales', () => {
  it('sin favoritos no hay sección; marcar uno la crea arriba y desmarcar la quita', async () => {
    await mount(<Panel />);
    expect(favSection()).toBeNull();
    const first = stars()[0]!;
    expect(first.getAttribute('aria-pressed')).toBe('false');
    await click(first);
    expect(favoriteIds()).toHaveLength(1);
    await vi.waitFor(() => expect(favSection()).not.toBeNull());
    // La sección de favoritos va antes que «Mis ajustes».
    const titles = [...document.querySelectorAll('h3.mat-section__title')].map((h) => h.textContent ?? '');
    expect(titles[0]).toContain('Favoritos');
    // Los favoritos suman «Aplicar a la selección».
    expect(favSection()!.textContent).toContain('Aplicar a la selección');
    await click(favSection()!.querySelector<HTMLButtonElement>('.mat-star')!);
    expect(favoriteIds()).toEqual([]);
    await vi.waitFor(() => expect(favSection()).toBeNull());
  });

  it('«Aplicar a la selección» mueve los objetos a una capa con esos ajustes', async () => {
    const id = useMaterials.getState().library.presets[0]!.id;
    s().setPref('favoriteMaterials', [id]);
    s().select(['obj-estrella']);
    await mount(<Panel />);
    const apply = [...favSection()!.querySelectorAll<HTMLButtonElement>('button')].find((b) => b.textContent === 'Aplicar a la selección')!;
    await click(apply);
    const layerColor = s().doc.objects.find((o) => o.id === 'obj-estrella')!.layer;
    expect(layerColor).not.toBe('#ffb020'); // ya no está en «Marcas»
    const preset = useMaterials.getState().library.presets[0]!;
    expect(s().doc.layers.find((l) => l.color === layerColor)).toMatchObject({ speed: preset.settings.speed, power: preset.settings.power });
  });

  it('sin selección, «Aplicar a la selección» explica por qué no se puede', async () => {
    s().setPref('favoriteMaterials', [useMaterials.getState().library.presets[0]!.id]);
    await mount(<Panel />);
    const apply = [...favSection()!.querySelectorAll<HTMLButtonElement>('button')].find((b) => b.textContent === 'Aplicar a la selección')!;
    expect(apply.getAttribute('aria-disabled')).toBe('true');
    expect(apply.title).toContain('Selecciona uno o más objetos');
  });

  it('borrar un material lo quita también de los favoritos', async () => {
    const id = useMaterials.getState().library.presets[0]!.id;
    s().setPref('favoriteMaterials', [id]);
    const done = deleteMaterial(id);
    await vi.waitFor(() => expect(usePrompts.getState().current).not.toBeNull());
    usePrompts.getState().answer('delete');
    expect(await done).toBe(true);
    expect(favoriteIds()).toEqual([]);
  });
});
