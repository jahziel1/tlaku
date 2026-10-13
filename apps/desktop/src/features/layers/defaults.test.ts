import { afterEach, describe, expect, it } from 'vitest';
import { getLayerDefaults, makeLayer, newLayerFor } from '@/lib/layers';
import { useStore } from '@/store';
import { resetLayerDefaults, saveLayerAsDefaults } from './defaults';

const RED = '#ff3b4a';

afterEach(() => {
  useStore.getState().resetPrefs();
  useStore.getState().newDocument();
});

describe('valores por defecto de capas nuevas (C35)', () => {
  it('guardar toma los ajustes de la capa; las capas nuevas empiezan con ellos', () => {
    const s = useStore.getState();
    s.ensureLayer(RED);
    s.updateLayer(RED, { speed: 2222, power: 31, name: 'Mi corte', order: 2 });
    saveLayerAsDefaults(useStore.getState(), RED);

    const saved = useStore.getState().prefs.layerDefaults;
    expect(saved).toMatchObject({ speed: 2222, power: 31 });
    expect(saved && ['color', 'name', 'order'].some((k) => k in saved)).toBe(false);
    expect(getLayerDefaults()).toEqual(saved);

    const fresh = newLayerFor('#2f7bff');
    expect(fresh).toMatchObject({ speed: 2222, power: 31, color: '#2f7bff', order: null });
    expect(fresh.name).not.toBe('Mi corte');
    expect(makeLayer('#000', 'x').speed).toBe(2222);
    // Las capas ya creadas no cambian.
    expect(useStore.getState().doc.layers[0]!.speed).toBe(2222);
    expect(useStore.getState().notices.at(-1)?.level).toBe('success');
  });

  it('restablecer vuelve a los valores de fábrica', () => {
    const s = useStore.getState();
    s.ensureLayer(RED);
    s.updateLayer(RED, { speed: 2222 });
    saveLayerAsDefaults(useStore.getState(), RED);
    resetLayerDefaults(useStore.getState());
    expect(useStore.getState().prefs.layerDefaults).toBeNull();
    expect(getLayerDefaults()).toBeNull();
    expect(newLayerFor('#2f7bff').speed).toBe(1000);
  });

  it('un cambio en las preferencias (leer el disco, importar un paquete) se refleja en makeLayer', () => {
    useStore.getState().replacePrefs({ prefs: { layerDefaults: { speed: 1800, mode: 'fill' } } });
    expect(makeLayer('#000', 'x')).toMatchObject({ speed: 1800, mode: 'fill' });
    useStore.getState().resetPrefs();
    expect(makeLayer('#000', 'x')).toMatchObject({ speed: 1000, mode: 'line' });
  });
});
